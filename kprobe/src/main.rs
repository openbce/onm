use std::{
    collections::BTreeMap,
    fmt,
    net::IpAddr,
    process::ExitCode,
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use clap::{Parser, ValueEnum};
use futures::{stream, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use k8s_openapi::{
    api::{
        apps::v1::DaemonSet,
        core::v1::{Namespace, Pod},
    },
    apimachinery::pkg::apis::meta::v1::ObjectMeta,
};
use kube::{
    api::{AttachParams, DeleteParams, ListParams, Patch, PatchParams, PostParams},
    config::KubeConfigOptions,
    Api, Client, Config, ResourceExt,
};
use serde_json::json;
use thiserror::Error;
use tokio::{io::AsyncReadExt, sync::Mutex};

const APP_LABEL: &str = "onm.openbce.io/kprobe";
const CONNECT_CONTAINER: &str = "connect";
const BANDWIDTH_CONTAINER: &str = "bandwidth";
const CONNECT_PORT: u16 = 1199;
const BANDWIDTH_PORT: u16 = 5201;
const MAX_FAILURE_SAMPLES: usize = 50;
const MAX_SLOWEST_SAMPLES: usize = 3;
const IPERF_BUSY_RETRIES: u32 = 8;
const IPERF_BUSY_BACKOFF: Duration = Duration::from_millis(250);

#[derive(Debug, Parser)]
#[command(
    name = "kprobe",
    version,
    about = "Check all directed Kubernetes pod-network paths"
)]
struct Args {
    /// Namespace in which to run the temporary DaemonSet
    #[arg(short, long, default_value = "onm-system")]
    namespace: String,

    /// Agnhost image used for connectivity checks
    #[arg(long, default_value = "registry.k8s.io/e2e-test-images/agnhost:2.61")]
    image: String,

    /// iperf3 image used for bandwidth measurement
    #[arg(long, default_value = "networkstatic/iperf3:multiarch")]
    bandwidth_image: String,

    /// Duration of each iperf3 bandwidth measurement (for example: 3s)
    #[arg(long, default_value = "3s", value_parser = parse_bandwidth_time)]
    bandwidth_time: Duration,

    /// Maximum number of simultaneous pod exec requests
    #[arg(short, long, default_value_t = 16, value_parser = parse_concurrency)]
    concurrency: usize,

    /// Pod IP address family to test
    #[arg(long, value_enum, default_value_t = IpFamily::Ipv4)]
    ip_family: IpFamily,

    /// Timeout for each connectivity check (for example: 3s or 500ms)
    #[arg(short, long, default_value = "5s", value_parser = parse_duration)]
    timeout: Duration,

    /// Maximum time to wait for all DaemonSet pods to become ready
    #[arg(long, default_value = "2m", value_parser = parse_duration)]
    ready_timeout: Duration,

    /// Additional node selector labels (comma-separated key=value pairs)
    #[arg(short = 'l', long = "selector", value_name = "SELECTOR", value_parser = parse_selector, default_value = "")]
    selector: BTreeMap<String, String>,
}

fn parse_duration(value: &str) -> Result<Duration, String> {
    humantime::parse_duration(value).map_err(|error| error.to_string())
}

fn parse_bandwidth_time(value: &str) -> Result<Duration, String> {
    let duration = parse_duration(value)?;
    if duration < Duration::from_secs(1) {
        return Err("bandwidth time must be at least 1s".into());
    }
    Ok(duration)
}

fn parse_concurrency(value: &str) -> Result<usize, String> {
    match value.parse::<usize>() {
        Ok(value) if value > 0 => Ok(value),
        _ => Err("concurrency must be greater than zero".into()),
    }
}

fn parse_selector(value: &str) -> Result<BTreeMap<String, String>, String> {
    let mut selector = BTreeMap::new();
    if value.trim().is_empty() {
        return Ok(selector);
    }

    for part in value.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (key, label_value) = part.split_once('=').ok_or_else(|| {
            format!("invalid selector {part:?}: expected key=value[,key=value...]")
        })?;
        let key = key.trim();
        let label_value = label_value.trim();
        if key.is_empty() {
            return Err(format!(
                "invalid selector {part:?}: label key must not be empty"
            ));
        }
        selector.insert(key.to_string(), label_value.to_string());
    }
    Ok(selector)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum IpFamily {
    Ipv4,
    Ipv6,
}

impl fmt::Display for IpFamily {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ipv4 => formatter.write_str("ipv4"),
            Self::Ipv6 => formatter.write_str("ipv6"),
        }
    }
}

impl IpFamily {
    fn matches(self, address: &IpAddr) -> bool {
        matches!(
            (self, address),
            (Self::Ipv4, IpAddr::V4(_)) | (Self::Ipv6, IpAddr::V6(_))
        )
    }
}

#[derive(Debug, Error)]
enum ProbeError {
    #[error("Kubernetes API error: {0}")]
    Kubernetes(#[from] kube::Error),
    #[error("the DaemonSet did not become ready within {0:?}")]
    ReadyTimeout(Duration),
    #[error("the DaemonSet has no eligible Linux nodes")]
    NoEligibleNodes,
    #[error("probe pod startup failed: {0}")]
    PodStartup(String),
    #[error("probe pod {pod} has no {family} address")]
    MissingPodIp { pod: String, family: IpFamily },
    #[error("interrupted")]
    Interrupted,
}

#[derive(Clone, Debug)]
struct Endpoint {
    pod: String,
    node: String,
    ip: String,
}

#[derive(Debug)]
struct TestResult {
    source: Endpoint,
    destination: Endpoint,
    port: u16,
    error: String,
}

#[derive(Debug)]
struct BandwidthResult {
    source: Endpoint,
    destination: Endpoint,
    bits_per_second: u64,
}

#[derive(Debug, Default)]
struct TestSummary {
    total: u64,
    passed_count: u64,
    connect_failed_count: u64,
    bandwidth_failed_count: u64,
    bandwidth_sum: u128,
    bandwidth_min: Option<u64>,
    bandwidth_max: Option<u64>,
    slowest: Vec<BandwidthResult>,
    failed: Vec<TestResult>,
}

impl TestSummary {
    fn failed_count(&self) -> u64 {
        self.connect_failed_count + self.bandwidth_failed_count
    }
}

struct Probe {
    daemonsets: Api<DaemonSet>,
    pods: Api<Pod>,
    name: String,
    selector: String,
}

async fn ensure_namespace(client: Client, name: &str) -> Result<(), kube::Error> {
    let namespaces: Api<Namespace> = Api::all(client);
    if namespaces.get_opt(name).await?.is_some() {
        return Ok(());
    }

    match namespaces
        .create(&PostParams::default(), &namespace(name))
        .await
    {
        Ok(_) => Ok(()),
        Err(kube::Error::Api(ref response)) if response.code == 409 => Ok(()),
        Err(error) => Err(error),
    }
}

fn namespace(name: &str) -> Namespace {
    Namespace {
        metadata: ObjectMeta {
            name: Some(name.to_string()),
            ..ObjectMeta::default()
        },
        ..Namespace::default()
    }
}

impl Probe {
    fn new(client: Client, namespace: &str) -> Self {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let name = format!("onm-kprobe-{suffix}-{}", std::process::id());
        Self {
            daemonsets: Api::namespaced(client.clone(), namespace),
            pods: Api::namespaced(client, namespace),
            selector: format!("{APP_LABEL}={name}"),
            name,
        }
    }

    async fn deploy(
        &self,
        connect_image: &str,
        bandwidth_image: &str,
        selector: &BTreeMap<String, String>,
    ) -> Result<(), ProbeError> {
        let daemonset = daemonset(&self.name, connect_image, bandwidth_image, selector);
        self.daemonsets
            .patch(
                &self.name,
                &PatchParams::apply("onm-kprobe"),
                &Patch::Apply(&daemonset),
            )
            .await?;
        Ok(())
    }

    async fn endpoints(
        &self,
        progress: &ProgressBar,
        timeout: Duration,
        family: IpFamily,
    ) -> Result<Vec<Endpoint>, ProbeError> {
        progress.set_message("waiting for probe pods");
        let started = Instant::now();
        let mut saw_status = false;

        loop {
            let daemonset = self.daemonsets.get(&self.name).await?;
            let pods = self
                .pods
                .list(&ListParams::default().labels(&self.selector))
                .await?;

            if let Some(status) = daemonset.status {
                saw_status = true;
                let desired = status.desired_number_scheduled;
                let ready_count = pods.items.iter().filter(|pod| pod_is_ready(pod)).count();
                progress.set_message(format!(
                    "waiting for probe pods ({ready_count}/{})",
                    desired.max(0) as usize
                ));

                // Probe pods tolerate every taint, including the cordon taint, so the
                // DaemonSet controller still creates pods for unschedulable nodes.
                // Those pods stay Pending; ignore them and proceed with ready pods.
                let created = pods.items.len() as i32;
                let all_resolved = pods
                    .items
                    .iter()
                    .all(|pod| pod_is_ready(pod) || pod_blocked_on_unschedulable_node(pod));
                if desired > 0 && created >= desired && all_resolved && ready_count > 0 {
                    break;
                }
                if desired > 0 && created >= desired && all_resolved && ready_count == 0 {
                    return Err(ProbeError::NoEligibleNodes);
                }
                if desired == 0 && started.elapsed() >= Duration::from_secs(5) {
                    return Err(ProbeError::NoEligibleNodes);
                }
            }

            if started.elapsed() >= Duration::from_secs(10) {
                if let Some(failure) = pods
                    .items
                    .iter()
                    .filter(|pod| !pod_blocked_on_unschedulable_node(pod))
                    .find_map(pod_startup_failure)
                {
                    return Err(ProbeError::PodStartup(failure));
                }
            }

            if started.elapsed() >= timeout {
                return if saw_status {
                    Err(ProbeError::ReadyTimeout(timeout))
                } else {
                    Err(ProbeError::NoEligibleNodes)
                };
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        let pods = self
            .pods
            .list(&ListParams::default().labels(&self.selector))
            .await?;
        let mut endpoints = Vec::with_capacity(pods.items.len());
        for pod in pods {
            if pod_blocked_on_unschedulable_node(&pod) {
                continue;
            }
            let pod_name = pod.name_any();
            let status = pod
                .status
                .as_ref()
                .ok_or_else(|| ProbeError::MissingPodIp {
                    pod: pod_name.clone(),
                    family,
                })?;
            let ip = select_pod_ip(status, family).ok_or_else(|| ProbeError::MissingPodIp {
                pod: pod_name.clone(),
                family,
            })?;
            endpoints.push(Endpoint {
                pod: pod_name,
                node: pod
                    .spec
                    .as_ref()
                    .and_then(|spec| spec.node_name.clone())
                    .unwrap_or_else(|| "<unknown>".into()),
                ip,
            });
        }
        if endpoints.is_empty() {
            return Err(ProbeError::NoEligibleNodes);
        }
        endpoints.sort_by(|left, right| left.node.cmp(&right.node));
        Ok(endpoints)
    }

    async fn run_tests(
        &self,
        endpoints: &[Endpoint],
        concurrency: usize,
        timeout: Duration,
        bandwidth_time: Duration,
        progress: &ProgressBar,
    ) -> TestSummary {
        let pairs: Vec<(Endpoint, Endpoint)> = endpoints
            .iter()
            .flat_map(|source| {
                endpoints
                    .iter()
                    .filter(move |destination| destination.pod != source.pod)
                    .map(move |destination| (source.clone(), destination.clone()))
            })
            .collect();
        let total = pairs.len() as u64;
        let mut summary = TestSummary {
            total,
            ..TestSummary::default()
        };

        progress.set_length(total);
        progress.set_position(0);
        progress.set_message("testing connectivity");

        let pods = self.pods.clone();
        let connect_results: Vec<(Endpoint, Endpoint, Result<(), String>)> = stream::iter(pairs)
            .map(|(source, destination)| {
                let pods = pods.clone();
                async move {
                    let result = test_connectivity(&pods, &source, &destination, timeout).await;
                    (source, destination, result)
                }
            })
            .buffer_unordered(concurrency)
            .inspect(|_| progress.inc(1))
            .collect()
            .await;

        let mut bandwidth_pairs = Vec::new();
        for (source, destination, result) in connect_results {
            match result {
                Ok(()) => bandwidth_pairs.push((source, destination)),
                Err(error) => {
                    summary.connect_failed_count += 1;
                    if summary.failed.len() < MAX_FAILURE_SAMPLES {
                        summary.failed.push(TestResult {
                            source,
                            destination,
                            port: CONNECT_PORT,
                            error,
                        });
                    }
                }
            }
        }

        progress.set_length(bandwidth_pairs.len() as u64);
        progress.set_position(0);
        progress.set_message("measuring bandwidth");

        let destination_locks: Arc<BTreeMap<String, Arc<Mutex<()>>>> = Arc::new(
            endpoints
                .iter()
                .map(|endpoint| (endpoint.pod.clone(), Arc::new(Mutex::new(()))))
                .collect(),
        );
        let pods = self.pods.clone();
        let bandwidth_results: Vec<(Endpoint, Endpoint, Result<u64, String>)> =
            stream::iter(bandwidth_pairs)
                .map(|(source, destination)| {
                    let pods = pods.clone();
                    let destination_lock = destination_locks
                        .get(&destination.pod)
                        .cloned()
                        .expect("every destination has a lock");
                    async move {
                        let _guard = destination_lock.lock().await;
                        let result =
                            test_bandwidth(&pods, &source, &destination, bandwidth_time).await;
                        (source, destination, result)
                    }
                })
                .buffer_unordered(concurrency)
                .inspect(|_| progress.inc(1))
                .collect()
                .await;

        for (source, destination, result) in bandwidth_results {
            match result {
                Ok(bits_per_second) => {
                    summary.passed_count += 1;
                    summary.bandwidth_sum += bits_per_second as u128;
                    summary.bandwidth_min = Some(
                        summary
                            .bandwidth_min
                            .map_or(bits_per_second, |current| current.min(bits_per_second)),
                    );
                    summary.bandwidth_max = Some(
                        summary
                            .bandwidth_max
                            .map_or(bits_per_second, |current| current.max(bits_per_second)),
                    );
                    record_slowest(
                        &mut summary.slowest,
                        BandwidthResult {
                            source,
                            destination,
                            bits_per_second,
                        },
                    );
                }
                Err(error) => {
                    summary.bandwidth_failed_count += 1;
                    if summary.failed.len() < MAX_FAILURE_SAMPLES {
                        summary.failed.push(TestResult {
                            source,
                            destination,
                            port: BANDWIDTH_PORT,
                            error,
                        });
                    }
                }
            }
        }

        progress.finish_and_clear();
        summary
    }

    async fn cleanup(&self) -> Result<(), kube::Error> {
        match self
            .daemonsets
            .delete(&self.name, &DeleteParams::default())
            .await
        {
            Ok(_) => Ok(()),
            Err(kube::Error::Api(ref response)) if response.code == 404 => Ok(()),
            Err(error) => Err(error),
        }
    }
}

fn pod_startup_failure(pod: &Pod) -> Option<String> {
    const FAILURE_REASONS: &[&str] = &[
        "CreateContainerConfigError",
        "CreateContainerError",
        "CrashLoopBackOff",
        "ErrImagePull",
        "ImagePullBackOff",
        "InvalidImageName",
        "RunContainerError",
    ];

    let status = pod.status.as_ref()?;
    let waiting = status
        .init_container_statuses
        .iter()
        .chain(status.container_statuses.iter())
        .flatten()
        .filter_map(|container| container.state.as_ref()?.waiting.as_ref())
        .find(|waiting| {
            waiting
                .reason
                .as_deref()
                .is_some_and(|reason| FAILURE_REASONS.contains(&reason))
        })?;
    let reason = waiting.reason.as_deref().unwrap_or("unknown error");
    let message = waiting.message.as_deref().unwrap_or("no details available");
    Some(format!(
        "{} on {}: {reason}: {message}",
        pod.name_any(),
        pod.spec
            .as_ref()
            .and_then(|spec| spec.node_name.as_deref())
            .unwrap_or("<unscheduled>")
    ))
}

fn pod_is_ready(pod: &Pod) -> bool {
    pod.status
        .as_ref()
        .and_then(|status| status.conditions.as_ref())
        .is_some_and(|conditions| {
            conditions
                .iter()
                .any(|condition| condition.type_ == "Ready" && condition.status == "True")
        })
}

fn pod_blocked_on_unschedulable_node(pod: &Pod) -> bool {
    let status = match pod.status.as_ref() {
        Some(status) => status,
        None => return false,
    };
    let condition = status.conditions.as_ref().and_then(|conditions| {
        conditions
            .iter()
            .find(|condition| condition.type_ == "PodScheduled")
    });
    match condition {
        Some(condition)
            if condition.status == "False"
                && condition.reason.as_deref() == Some("Unschedulable") =>
        {
            condition
                .message
                .as_deref()
                .is_some_and(|message| message.to_ascii_lowercase().contains("unschedulable"))
        }
        _ => false,
    }
}

fn select_pod_ip(
    status: &k8s_openapi::api::core::v1::PodStatus,
    family: IpFamily,
) -> Option<String> {
    status
        .pod_ips
        .iter()
        .flatten()
        .map(|pod_ip| &pod_ip.ip)
        .chain(status.pod_ip.iter())
        .find(|ip| IpAddr::from_str(ip).is_ok_and(|address| family.matches(&address)))
        .cloned()
}

fn node_selector(extra: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let mut selector = BTreeMap::from([("kubernetes.io/os".into(), "linux".into())]);
    selector.extend(
        extra
            .iter()
            .map(|(key, value)| (key.clone(), value.clone())),
    );
    selector
}

fn daemonset(
    name: &str,
    connect_image: &str,
    bandwidth_image: &str,
    selector: &BTreeMap<String, String>,
) -> DaemonSet {
    serde_json::from_value(json!({
        "apiVersion": "apps/v1",
        "kind": "DaemonSet",
        "metadata": {
            "name": name,
            "labels": { (APP_LABEL): name }
        },
        "spec": {
            "selector": { "matchLabels": { (APP_LABEL): name } },
            "template": {
                "metadata": { "labels": { (APP_LABEL): name } },
                "spec": {
                    "automountServiceAccountToken": false,
                    "nodeSelector": node_selector(selector),
                    "terminationGracePeriodSeconds": 0,
                    "tolerations": [{ "operator": "Exists" }],
                    "volumes": [{ "name": "iperf-tmp", "emptyDir": {} }],
                    "containers": [
                        {
                            "name": CONNECT_CONTAINER,
                            "image": connect_image,
                            "args": ["netexec", format!("--http-port={CONNECT_PORT}"), "--udp-port=-1"],
                            "ports": [{ "name": "connect", "containerPort": CONNECT_PORT, "protocol": "TCP" }],
                            "readinessProbe": {
                                "httpGet": { "path": "/", "port": CONNECT_PORT },
                                "periodSeconds": 1,
                                "timeoutSeconds": 1
                            },
                            "securityContext": {
                                "allowPrivilegeEscalation": false,
                                "capabilities": { "drop": ["ALL"] },
                                "readOnlyRootFilesystem": true
                            }
                        },
                        {
                            "name": BANDWIDTH_CONTAINER,
                            "image": bandwidth_image,
                            "args": ["-s", "-p", BANDWIDTH_PORT.to_string()],
                            "workingDir": "/tmp",
                            "ports": [{ "name": "bandwidth", "containerPort": BANDWIDTH_PORT, "protocol": "TCP" }],
                            "volumeMounts": [{ "name": "iperf-tmp", "mountPath": "/tmp" }],
                            "env": [{ "name": "TMPDIR", "value": "/tmp" }],
                            "readinessProbe": {
                                "tcpSocket": { "port": BANDWIDTH_PORT },
                                "periodSeconds": 1,
                                "timeoutSeconds": 1
                            },
                            "securityContext": {
                                "allowPrivilegeEscalation": false,
                                "capabilities": { "drop": ["ALL"] },
                                "readOnlyRootFilesystem": true
                            }
                        }
                    ],
                    "securityContext": { "seccompProfile": { "type": "RuntimeDefault" } }
                }
            }
        }
    }))
    .expect("the built-in DaemonSet manifest must be valid")
}

fn format_host_port(ip: &str, port: u16) -> String {
    if ip.contains(':') {
        format!("[{ip}]:{port}")
    } else {
        format!("{ip}:{port}")
    }
}

fn json_bits_per_second(value: &serde_json::Value) -> Option<u64> {
    value
        .as_f64()
        .or_else(|| value.as_u64().map(|bits| bits as f64))
        .filter(|bits| bits.is_finite() && *bits >= 0.0)
        .map(|bits| bits.round() as u64)
}

fn parse_iperf_bandwidth(stdout: &str, stderr: &str) -> Result<u64, String> {
    let trimmed = stdout.trim();
    let value: serde_json::Value = serde_json::from_str(trimmed).map_err(|error| {
        let detail = [stderr.trim(), trimmed]
            .into_iter()
            .find(|message| !message.is_empty())
            .unwrap_or("iperf3 produced no JSON output");
        format!("invalid iperf3 JSON ({error}): {detail}")
    })?;

    if let Some(error) = value.get("error").and_then(|error| error.as_str()) {
        return Err(error.to_string());
    }

    value
        .pointer("/end/sum_received/bits_per_second")
        .or_else(|| value.pointer("/end/sum_sent/bits_per_second"))
        .or_else(|| value.pointer("/end/streams/0/receiver/bits_per_second"))
        .or_else(|| value.pointer("/end/streams/0/sender/bits_per_second"))
        .and_then(json_bits_per_second)
        .ok_or_else(|| "iperf3 JSON did not include bits_per_second".into())
}

fn record_slowest(samples: &mut Vec<BandwidthResult>, sample: BandwidthResult) {
    samples.push(sample);
    samples.sort_by_key(|entry| entry.bits_per_second);
    if samples.len() > MAX_SLOWEST_SAMPLES {
        samples.truncate(MAX_SLOWEST_SAMPLES);
    }
}

fn format_bandwidth(bits_per_second: u64) -> String {
    const MEGABIT: f64 = 1_000_000.0;
    const GIGABIT: f64 = 1_000_000_000.0;
    let bits = bits_per_second as f64;
    if bits >= GIGABIT {
        format!("{:.2} Gbit/s", bits / GIGABIT)
    } else if bits >= MEGABIT {
        format!("{:.2} Mbit/s", bits / MEGABIT)
    } else if bits >= 1_000.0 {
        format!("{:.2} Kbit/s", bits / 1_000.0)
    } else {
        format!("{bits_per_second} bit/s")
    }
}

struct ExecOutput {
    stdout: String,
    stderr: String,
    status: Option<String>,
}

async fn exec_in_container(
    pods: &Api<Pod>,
    pod: &str,
    container: &str,
    command: Vec<&str>,
    timeout: Duration,
) -> Result<ExecOutput, String> {
    let params = AttachParams::default().container(container);
    let operation = async {
        let mut process = pods
            .exec(pod, command, &params)
            .await
            .map_err(|error| error.to_string())?;
        let status = process.take_status();
        let mut stdout = process
            .stdout()
            .ok_or_else(|| "exec stdout was not attached".to_string())?;
        let mut stderr = process
            .stderr()
            .ok_or_else(|| "exec stderr was not attached".to_string())?;
        let mut stdout_bytes = Vec::new();
        let mut stderr_bytes = Vec::new();
        let (stdout_result, stderr_result) = tokio::join!(
            stdout.read_to_end(&mut stdout_bytes),
            stderr.read_to_end(&mut stderr_bytes)
        );
        stdout_result.map_err(|error| error.to_string())?;
        stderr_result.map_err(|error| error.to_string())?;
        let remote_status = match status {
            Some(status) => status.await,
            None => None,
        };
        process.join().await.map_err(|error| error.to_string())?;

        Ok(ExecOutput {
            stdout: String::from_utf8_lossy(&stdout_bytes).into_owned(),
            stderr: String::from_utf8_lossy(&stderr_bytes).into_owned(),
            status: remote_status.and_then(|status| status.status),
        })
    };

    tokio::time::timeout(timeout + Duration::from_secs(10), operation)
        .await
        .map_err(|_| {
            format!(
                "exec timed out after {:?}",
                timeout + Duration::from_secs(10)
            )
        })?
}

async fn test_connectivity(
    pods: &Api<Pod>,
    source: &Endpoint,
    destination: &Endpoint,
    timeout: Duration,
) -> Result<(), String> {
    let address = format_host_port(&destination.ip, CONNECT_PORT);
    let command_timeout = humantime::format_duration(timeout).to_string();
    let command = vec![
        "/agnhost",
        "connect",
        "--timeout",
        &command_timeout,
        &address,
    ];
    let output = exec_in_container(pods, &source.pod, CONNECT_CONTAINER, command, timeout).await?;

    if output.status.as_deref() == Some("Success") {
        return Ok(());
    }

    let stderr = output.stderr.trim().to_string();
    let stdout = output.stdout.trim().to_string();
    let status_message = output.status.unwrap_or_default();
    Err([stderr, stdout, status_message]
        .into_iter()
        .find(|message| !message.is_empty())
        .unwrap_or_else(|| "remote connect command failed".into()))
}

fn is_iperf_server_busy(error: &str) -> bool {
    error.to_ascii_lowercase().contains("server is busy")
}

async fn test_bandwidth(
    pods: &Api<Pod>,
    source: &Endpoint,
    destination: &Endpoint,
    bandwidth_time: Duration,
) -> Result<u64, String> {
    let mut attempt = 0;
    loop {
        match run_iperf_client(pods, source, destination, bandwidth_time).await {
            Ok(bits_per_second) => return Ok(bits_per_second),
            Err(error) if is_iperf_server_busy(&error) && attempt < IPERF_BUSY_RETRIES => {
                attempt += 1;
                tokio::time::sleep(IPERF_BUSY_BACKOFF.saturating_mul(attempt)).await;
            }
            Err(error) => return Err(error),
        }
    }
}

async fn run_iperf_client(
    pods: &Api<Pod>,
    source: &Endpoint,
    destination: &Endpoint,
    bandwidth_time: Duration,
) -> Result<u64, String> {
    let seconds = bandwidth_time.as_secs().max(1).to_string();
    let port = BANDWIDTH_PORT.to_string();
    let mut command = vec![
        "iperf3",
        "-c",
        destination.ip.as_str(),
        "-p",
        &port,
        "-t",
        &seconds,
        "-J",
    ];
    if destination.ip.contains(':') {
        command.push("-6");
    }

    let output = exec_in_container(
        pods,
        &source.pod,
        BANDWIDTH_CONTAINER,
        command,
        bandwidth_time + Duration::from_secs(15),
    )
    .await?;

    match parse_iperf_bandwidth(&output.stdout, &output.stderr) {
        Ok(bits_per_second) => Ok(bits_per_second),
        Err(error) if output.status.as_deref() == Some("Success") => Err(error),
        Err(error) => {
            let stderr = output.stderr.trim();
            if !stderr.is_empty() {
                Err(format!("{error}: {stderr}"))
            } else {
                Err(error)
            }
        }
    }
}

fn progress_bar() -> ProgressBar {
    let progress = ProgressBar::new_spinner();
    progress.set_style(
        ProgressStyle::with_template(
            "{spinner:.cyan} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}",
        )
        .expect("valid progress template")
        .progress_chars("=>-"),
    );
    progress.enable_steady_tick(Duration::from_millis(100));
    progress
}

fn print_summary(endpoints: &[Endpoint], summary: &TestSummary, elapsed: Duration) {
    println!("Network probe complete");
    println!("  Pods:       {}", endpoints.len());
    println!("  Paths:      {}", summary.total);
    println!(
        "  Connect:    {} passed, {} failed",
        summary.total - summary.connect_failed_count,
        summary.connect_failed_count
    );
    let bandwidth_attempted = summary.passed_count + summary.bandwidth_failed_count;
    println!(
        "  Bandwidth:  {} passed, {} failed",
        summary.passed_count, summary.bandwidth_failed_count
    );
    if summary.passed_count > 0 {
        let average = summary.bandwidth_sum / summary.passed_count as u128;
        print!("  Throughput: avg {}", format_bandwidth(average as u64));
        if let (Some(minimum), Some(maximum)) = (summary.bandwidth_min, summary.bandwidth_max) {
            println!(
                " (min {}, max {})",
                format_bandwidth(minimum),
                format_bandwidth(maximum)
            );
        } else {
            println!();
        }
    } else if bandwidth_attempted == 0 {
        println!("  Throughput: n/a");
    }
    println!("  Elapsed:    {:.2?}", elapsed);

    if !summary.slowest.is_empty() {
        println!(
            "\nSlowest paths (top {} of {}):",
            summary.slowest.len(),
            summary.passed_count
        );
        for result in &summary.slowest {
            println!(
                "  {} ({}) -> {} ({}, {}:{}): {}",
                result.source.pod,
                result.source.node,
                result.destination.pod,
                result.destination.node,
                result.destination.ip,
                BANDWIDTH_PORT,
                format_bandwidth(result.bits_per_second)
            );
        }
    }

    if !summary.failed.is_empty() {
        println!(
            "\nFailed paths (showing {} of {}):",
            summary.failed.len(),
            summary.failed_count()
        );
        for result in &summary.failed {
            println!(
                "  {} ({}) -> {} ({}, {}:{}): {}",
                result.source.pod,
                result.source.node,
                result.destination.pod,
                result.destination.node,
                result.destination.ip,
                result.port,
                result.error
            );
        }
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();
    let started = Instant::now();

    let config = match Config::from_kubeconfig(&KubeConfigOptions::default()).await {
        Ok(config) => config,
        Err(error) => {
            eprintln!("kprobe: could not load the current kubeconfig context: {error}");
            return ExitCode::FAILURE;
        }
    };
    let client = match Client::try_from(config) {
        Ok(client) => client,
        Err(error) => {
            eprintln!("kprobe: could not create Kubernetes client: {error}");
            return ExitCode::FAILURE;
        }
    };

    let progress = progress_bar();
    progress.set_message(format!("ensuring namespace {}", args.namespace));
    if let Err(error) = ensure_namespace(client.clone(), &args.namespace).await {
        progress.finish_and_clear();
        eprintln!(
            "kprobe: could not ensure namespace {}: {error}",
            args.namespace
        );
        return ExitCode::FAILURE;
    }

    let probe = Probe::new(client, &args.namespace);
    if let Err(error) = probe
        .deploy(&args.image, &args.bandwidth_image, &args.selector)
        .await
    {
        progress.finish_and_clear();
        eprintln!("kprobe: {error}");
        return ExitCode::FAILURE;
    }

    let work = async {
        let endpoints = probe
            .endpoints(&progress, args.ready_timeout, args.ip_family)
            .await?;
        let summary = probe
            .run_tests(
                &endpoints,
                args.concurrency,
                args.timeout,
                args.bandwidth_time,
                &progress,
            )
            .await;
        Ok::<_, ProbeError>((endpoints, summary))
    };

    let outcome = tokio::select! {
        result = work => result,
        _ = tokio::signal::ctrl_c() => Err(ProbeError::Interrupted),
    };
    progress.finish_and_clear();
    let cleanup = probe.cleanup().await;

    match outcome {
        Ok((endpoints, summary)) => {
            print_summary(&endpoints, &summary, started.elapsed());
            if let Err(error) = cleanup {
                eprintln!("kprobe: failed to remove temporary DaemonSet: {error}");
                return ExitCode::FAILURE;
            }
            if summary.failed_count() > 0 {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(error) => {
            eprintln!("kprobe: {error}");
            if let Err(cleanup_error) = cleanup {
                eprintln!("kprobe: failed to remove temporary DaemonSet: {cleanup_error}");
            }
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::{PodIP, PodStatus};

    #[test]
    fn uses_onm_system_by_default() {
        let args = Args::try_parse_from(["kprobe"]).expect("default arguments");
        assert_eq!(args.namespace, "onm-system");
        assert_eq!(args.image, "registry.k8s.io/e2e-test-images/agnhost:2.61");
        assert_eq!(args.bandwidth_image, "networkstatic/iperf3:multiarch");
        assert_eq!(args.bandwidth_time, Duration::from_secs(3));
        assert!(args.selector.is_empty());
    }

    #[test]
    fn parses_bandwidth_time() {
        let args = Args::try_parse_from(["kprobe", "--bandwidth-time", "5s"])
            .expect("bandwidth time argument");
        assert_eq!(args.bandwidth_time, Duration::from_secs(5));
    }

    #[test]
    fn rejects_subsecond_bandwidth_time() {
        let error = parse_bandwidth_time("500ms").expect_err("subsecond");
        assert!(error.contains("at least 1s"));
    }

    #[test]
    fn parses_iperf_json_bandwidth() {
        let stdout = r#"{
            "end": {
                "sum_received": { "bits_per_second": 125000000.4 },
                "sum_sent": { "bits_per_second": 124000000.0 }
            }
        }"#;
        assert_eq!(parse_iperf_bandwidth(stdout, "").expect("bps"), 125_000_000);
        assert_eq!(
            parse_iperf_bandwidth(r#"{"error":"connection refused"}"#, "")
                .expect_err("iperf error"),
            "connection refused"
        );
    }

    #[test]
    fn detects_busy_iperf_server_errors() {
        assert!(is_iperf_server_busy(
            "the server is busy running a test. try again later"
        ));
        assert!(!is_iperf_server_busy("connection refused"));
    }

    #[test]
    fn formats_bandwidth_values() {
        assert_eq!(format_bandwidth(12_500_000), "12.50 Mbit/s");
        assert_eq!(format_bandwidth(1_500_000_000), "1.50 Gbit/s");
    }

    #[test]
    fn tracks_only_top_three_slowest_bandwidth_samples() {
        let source = Endpoint {
            pod: "probe-a".into(),
            node: "node-a".into(),
            ip: "10.0.0.1".into(),
        };
        let destination = Endpoint {
            pod: "probe-b".into(),
            node: "node-b".into(),
            ip: "10.0.0.2".into(),
        };
        let mut samples = Vec::new();
        for bits_per_second in [40_000_000, 10_000_000, 30_000_000, 20_000_000] {
            record_slowest(
                &mut samples,
                BandwidthResult {
                    source: source.clone(),
                    destination: destination.clone(),
                    bits_per_second,
                },
            );
        }
        assert_eq!(samples.len(), 3);
        assert_eq!(
            samples
                .iter()
                .map(|sample| sample.bits_per_second)
                .collect::<Vec<_>>(),
            vec![10_000_000, 20_000_000, 30_000_000]
        );
    }

    #[test]
    fn parses_node_selector_labels() {
        let args = Args::try_parse_from([
            "kprobe",
            "--selector",
            "kubernetes.io/hostname=node-a,topology.kubernetes.io/zone=zone-1",
        ])
        .expect("selector arguments");
        assert_eq!(
            args.selector
                .get("kubernetes.io/hostname")
                .map(String::as_str),
            Some("node-a")
        );
        assert_eq!(
            args.selector
                .get("topology.kubernetes.io/zone")
                .map(String::as_str),
            Some("zone-1")
        );
    }

    #[test]
    fn rejects_invalid_node_selector() {
        let error = parse_selector("hostname").expect_err("missing equals");
        assert!(error.contains("expected key=value"));
    }

    #[test]
    fn namespace_manifest_uses_selected_name() {
        let namespace = namespace("onm-system");
        assert_eq!(namespace.metadata.name.as_deref(), Some("onm-system"));
    }

    #[test]
    fn selects_the_requested_address_family() {
        let status = PodStatus {
            pod_ip: Some("fd00::2".into()),
            pod_ips: Some(vec![
                PodIP {
                    ip: "fd00::2".into(),
                },
                PodIP {
                    ip: "10.0.0.2".into(),
                },
            ]),
            ..PodStatus::default()
        };
        assert_eq!(
            select_pod_ip(&status, IpFamily::Ipv4).as_deref(),
            Some("10.0.0.2")
        );
        assert_eq!(
            select_pod_ip(&status, IpFamily::Ipv6).as_deref(),
            Some("fd00::2")
        );
    }

    #[test]
    fn reports_fatal_pod_startup_state() {
        let pod: Pod = serde_json::from_value(json!({
            "metadata": { "name": "probe-a" },
            "spec": {
                "nodeName": "node-a",
                "containers": [{ "name": "connect", "image": "missing:test" }]
            },
            "status": {
                "containerStatuses": [{
                    "name": "connect",
                    "image": "missing:test",
                    "imageID": "",
                    "ready": false,
                    "restartCount": 0,
                    "started": false,
                    "state": {
                        "waiting": {
                            "reason": "ImagePullBackOff",
                            "message": "image not found"
                        }
                    }
                }]
            }
        }))
        .expect("pod");

        assert_eq!(
            pod_startup_failure(&pod).as_deref(),
            Some("probe-a on node-a: ImagePullBackOff: image not found")
        );
    }

    #[test]
    fn detects_pods_blocked_on_unschedulable_nodes() {
        let pod: Pod = serde_json::from_value(json!({
            "metadata": { "name": "probe-b" },
            "spec": {
                "containers": [{ "name": "connect", "image": "example/agnhost:test" }]
            },
            "status": {
                "phase": "Pending",
                "conditions": [{
                    "type": "PodScheduled",
                    "status": "False",
                    "reason": "Unschedulable",
                    "message": "0/1 nodes are available: 1 node(s) were unschedulable."
                }]
            }
        }))
        .expect("pod");

        assert!(pod_blocked_on_unschedulable_node(&pod));
        assert!(!pod_is_ready(&pod));
    }

    #[test]
    fn formats_ip_addresses_for_connect() {
        assert_eq!(format_host_port("10.0.0.2", 1199), "10.0.0.2:1199");
        assert_eq!(format_host_port("fd00::2", 1199), "[fd00::2]:1199");
    }

    #[test]
    fn daemonset_is_scoped_and_self_contained() {
        let selector = BTreeMap::from([("kubernetes.io/hostname".into(), "node-a".into())]);
        let daemonset = daemonset(
            "onm-kprobe-test",
            "registry.k8s.io/e2e-test-images/agnhost:2.61",
            "networkstatic/iperf3:multiarch",
            &selector,
        );
        assert_eq!(
            daemonset.metadata.labels.as_ref().unwrap().get(APP_LABEL),
            Some(&"onm-kprobe-test".to_string())
        );
        let spec = daemonset.spec.expect("spec");
        let pod_spec = spec.template.spec.expect("pod spec");
        let node_selector = pod_spec.node_selector.unwrap();
        assert_eq!(
            node_selector.get("kubernetes.io/os"),
            Some(&"linux".to_string())
        );
        assert_eq!(
            node_selector.get("kubernetes.io/hostname"),
            Some(&"node-a".to_string())
        );
        assert!(pod_spec.affinity.is_none());
        assert_eq!(pod_spec.containers.len(), 2);

        let connect = &pod_spec.containers[0];
        assert_eq!(connect.name, CONNECT_CONTAINER);
        assert_eq!(
            connect.args.as_ref().map(|args| args.as_slice()),
            Some(
                [
                    "netexec".to_string(),
                    "--http-port=1199".to_string(),
                    "--udp-port=-1".to_string()
                ]
                .as_slice()
            )
        );

        let bandwidth = &pod_spec.containers[1];
        assert_eq!(bandwidth.name, BANDWIDTH_CONTAINER);
        assert_eq!(
            bandwidth.args.as_ref().map(|args| args.as_slice()),
            Some(["-s".to_string(), "-p".to_string(), "5201".to_string()].as_slice())
        );
        assert_eq!(
            bandwidth
                .volume_mounts
                .as_ref()
                .and_then(|mounts| mounts.first())
                .map(|mount| (mount.name.as_str(), mount.mount_path.as_str())),
            Some(("iperf-tmp", "/tmp"))
        );
        assert!(
            connect.resources.is_none() && bandwidth.resources.is_none(),
            "probe pods should have BestEffort QoS"
        );
        assert!(!pod_spec.automount_service_account_token.unwrap());
    }
}
