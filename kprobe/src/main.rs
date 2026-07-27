use std::{
    fmt,
    net::IpAddr,
    process::ExitCode,
    str::FromStr,
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
    Api, Client, ResourceExt,
};
use serde_json::json;
use thiserror::Error;
use tokio::io::AsyncReadExt;

const APP_LABEL: &str = "onm.openbce.io/kprobe";
const CONTAINER: &str = "agnhost";
const PORT: u16 = 1199;
const MAX_FAILURE_SAMPLES: usize = 50;

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

    /// Agnhost image used by the probe
    #[arg(long, default_value = "registry.k8s.io/e2e-test-images/agnhost:2.66")]
    image: String,

    /// Maximum number of simultaneous pod exec requests
    #[arg(short, long, default_value_t = 16, value_parser = parse_concurrency)]
    concurrency: usize,

    /// Pod IP address family to test
    #[arg(long, value_enum, default_value_t = IpFamily::Ipv4)]
    ip_family: IpFamily,

    /// Timeout for each network connection (for example: 3s or 500ms)
    #[arg(short, long, default_value = "5s", value_parser = parse_duration)]
    timeout: Duration,

    /// Maximum time to wait for all DaemonSet pods to become ready
    #[arg(long, default_value = "2m", value_parser = parse_duration)]
    ready_timeout: Duration,
}

fn parse_duration(value: &str) -> Result<Duration, String> {
    humantime::parse_duration(value).map_err(|error| error.to_string())
}

fn parse_concurrency(value: &str) -> Result<usize, String> {
    match value.parse::<usize>() {
        Ok(value) if value > 0 => Ok(value),
        _ => Err("concurrency must be greater than zero".into()),
    }
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
    error: String,
}

#[derive(Debug)]
struct TestSummary {
    total: u64,
    failed_count: u64,
    failed: Vec<TestResult>,
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

    async fn deploy(&self, image: &str) -> Result<(), ProbeError> {
        let daemonset = daemonset(&self.name, image);
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
            if let Some(status) = daemonset.status {
                saw_status = true;
                let desired = status.desired_number_scheduled;
                let ready = status.number_ready;
                progress.set_message(format!("waiting for probe pods ({ready}/{desired})"));
                if desired > 0 && ready == desired {
                    break;
                }
                if desired == 0 && started.elapsed() >= Duration::from_secs(5) {
                    return Err(ProbeError::NoEligibleNodes);
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
        endpoints.sort_by(|left, right| left.node.cmp(&right.node));
        Ok(endpoints)
    }

    async fn run_tests(
        &self,
        endpoints: &[Endpoint],
        concurrency: usize,
        timeout: Duration,
        progress: &ProgressBar,
    ) -> TestSummary {
        let total = endpoints
            .len()
            .saturating_mul(endpoints.len().saturating_sub(1)) as u64;
        progress.set_length(total);
        progress.set_position(0);
        progress.set_message("testing pod paths");

        let pods = self.pods.clone();
        let pairs = endpoints.iter().flat_map(|source| {
            endpoints
                .iter()
                .filter(move |destination| destination.pod != source.pod)
                .map(move |destination| (source.clone(), destination.clone()))
        });
        let summary = stream::iter(pairs)
            .map(|(source, destination)| {
                let pods = pods.clone();
                async move {
                    test_path(&pods, &source, &destination, timeout)
                        .await
                        .err()
                        .map(|error| TestResult {
                            source,
                            destination,
                            error,
                        })
                }
            })
            .buffer_unordered(concurrency)
            .inspect(|_| progress.inc(1))
            .fold(
                TestSummary {
                    total,
                    failed_count: 0,
                    failed: Vec::new(),
                },
                |mut summary, result| async move {
                    if let Some(result) = result {
                        summary.failed_count += 1;
                        if summary.failed.len() < MAX_FAILURE_SAMPLES {
                            summary.failed.push(result);
                        }
                    }
                    summary
                },
            )
            .await;
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

fn daemonset(name: &str, image: &str) -> DaemonSet {
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
                    "nodeSelector": { "kubernetes.io/os": "linux" },
                    "terminationGracePeriodSeconds": 0,
                    "tolerations": [{ "operator": "Exists" }],
                    "containers": [{
                        "name": CONTAINER,
                        "image": image,
                        "args": ["netexec", format!("--http-port={PORT}"), "--udp-port=-1"],
                        "ports": [{ "name": "http", "containerPort": PORT, "protocol": "TCP" }],
                        "readinessProbe": {
                            "httpGet": { "path": "/", "port": PORT },
                            "periodSeconds": 1,
                            "timeoutSeconds": 1
                        },
                        "resources": {
                            "requests": { "cpu": "5m", "memory": "8Mi" },
                            "limits": { "memory": "32Mi" }
                        },
                        "securityContext": {
                            "allowPrivilegeEscalation": false,
                            "capabilities": { "drop": ["ALL"] },
                            "readOnlyRootFilesystem": true
                        }
                    }],
                    "securityContext": { "seccompProfile": { "type": "RuntimeDefault" } }
                }
            }
        }
    }))
    .expect("the built-in DaemonSet manifest must be valid")
}

async fn test_path(
    pods: &Api<Pod>,
    source: &Endpoint,
    destination: &Endpoint,
    timeout: Duration,
) -> Result<(), String> {
    let address = format_host_port(&destination.ip, PORT);
    let command_timeout = humantime::format_duration(timeout).to_string();
    let command = vec![
        "/agnhost",
        "connect",
        "--timeout",
        &command_timeout,
        &address,
    ];
    let params = AttachParams::default().container(CONTAINER);

    let operation = async {
        let mut process = pods
            .exec(&source.pod, command, &params)
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

        if remote_status
            .as_ref()
            .and_then(|status| status.status.as_deref())
            == Some("Success")
        {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&stderr_bytes).trim().to_string();
        let stdout = String::from_utf8_lossy(&stdout_bytes).trim().to_string();
        let status_message = remote_status.and_then(|status| status.message);
        Err([stderr, stdout, status_message.unwrap_or_default()]
            .into_iter()
            .find(|message| !message.is_empty())
            .unwrap_or_else(|| "remote connect command failed".into()))
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

fn format_host_port(ip: &str, port: u16) -> String {
    if ip.contains(':') {
        format!("[{ip}]:{port}")
    } else {
        format!("{ip}:{port}")
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
    println!("Connectivity probe complete");
    println!("  Pods:    {}", endpoints.len());
    println!("  Paths:   {}", summary.total);
    println!("  Passed:  {}", summary.total - summary.failed_count);
    println!("  Failed:  {}", summary.failed_count);
    println!("  Elapsed: {:.2?}", elapsed);

    if !summary.failed.is_empty() {
        println!(
            "\nFailed paths (showing {} of {}):",
            summary.failed.len(),
            summary.failed_count
        );
        for result in &summary.failed {
            println!(
                "  {} ({}) -> {} ({}, {}:{}): {}",
                result.source.pod,
                result.source.node,
                result.destination.pod,
                result.destination.node,
                result.destination.ip,
                PORT,
                result.error
            );
        }
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();
    let started = Instant::now();
    let progress = progress_bar();

    let client = match Client::try_default().await {
        Ok(client) => client,
        Err(error) => {
            progress.finish_and_clear();
            eprintln!("kprobe: could not load Kubernetes configuration: {error}");
            return ExitCode::FAILURE;
        }
    };
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
    if let Err(error) = probe.deploy(&args.image).await {
        progress.finish_and_clear();
        eprintln!("kprobe: {error}");
        return ExitCode::FAILURE;
    }

    let work = async {
        let endpoints = probe
            .endpoints(&progress, args.ready_timeout, args.ip_family)
            .await?;
        let summary = probe
            .run_tests(&endpoints, args.concurrency, args.timeout, &progress)
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
            if summary.failed_count > 0 {
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
    }

    #[test]
    fn namespace_manifest_uses_selected_name() {
        let namespace = namespace("onm-system");
        assert_eq!(namespace.metadata.name.as_deref(), Some("onm-system"));
    }

    #[test]
    fn formats_ip_addresses_for_connect() {
        assert_eq!(format_host_port("10.0.0.2", 1199), "10.0.0.2:1199");
        assert_eq!(format_host_port("fd00::2", 1199), "[fd00::2]:1199");
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
    fn daemonset_is_scoped_and_self_contained() {
        let daemonset = daemonset("onm-kprobe-test", "example/agnhost:test");
        assert_eq!(
            daemonset.metadata.labels.as_ref().unwrap().get(APP_LABEL),
            Some(&"onm-kprobe-test".to_string())
        );
        let spec = daemonset.spec.expect("spec");
        let pod_spec = spec.template.spec.expect("pod spec");
        assert_eq!(
            pod_spec.node_selector.unwrap().get("kubernetes.io/os"),
            Some(&"linux".to_string())
        );
        assert_eq!(
            pod_spec.containers[0].args.as_ref().unwrap()[1],
            "--http-port=1199"
        );
        assert!(!pod_spec.automount_service_account_token.unwrap());
    }
}
