use comfy_table::{presets::UTF8_FULL, Table};
use libonm::eth::{self, EthError};

pub fn run(name: &str) -> Result<(), EthError> {
    let iface = eth::get_interface(name)?;

    let mut iface_table = Table::new();
    iface_table.load_preset(UTF8_FULL);
    iface_table.set_header(vec!["Interface Property", "Value"]);
    iface_table.add_row(vec!["Name", &iface.name]);
    iface_table.add_row(vec!["MAC Address", &iface.mac_address]);
    iface_table.add_row(vec!["MTU", &iface.mtu.to_string()]);
    iface_table.add_row(vec!["State", &iface.state.to_string()]);
    iface_table.add_row(vec![
        "Speed",
        &iface
            .speed
            .map(|s| format!("{} Mbps", s))
            .unwrap_or("-".to_string()),
    ]);
    iface_table.add_row(vec!["Driver", &iface.driver.clone().unwrap_or("-".to_string())]);
    iface_table.add_row(vec!["PCI Slot", &iface.pci_slot.clone().unwrap_or("-".to_string())]);

    println!("{iface_table}");
    println!();

    print_sysctl_tables();

    Ok(())
}

pub fn print_sysctl_tables() {
    use libonm::eth;

    let sysctl = eth::get_network_sysctl();

    let mut conntrack = Table::new();
    conntrack.load_preset(UTF8_FULL);
    conntrack.set_header(vec!["Connection Tracking", "Value"]);
    add_row_opt(&mut conntrack, "nf_conntrack_max", sysctl.conntrack.max);
    add_row_opt(&mut conntrack, "nf_conntrack_buckets", sysctl.conntrack.buckets);
    add_row_opt(
        &mut conntrack,
        "nf_conntrack_tcp_timeout_established",
        sysctl.conntrack.tcp_timeout_established,
    );
    add_row_opt(
        &mut conntrack,
        "nf_conntrack_tcp_timeout_time_wait",
        sysctl.conntrack.tcp_timeout_time_wait,
    );
    add_row_opt(
        &mut conntrack,
        "nf_conntrack_tcp_timeout_close_wait",
        sysctl.conntrack.tcp_timeout_close_wait,
    );
    add_row_opt(
        &mut conntrack,
        "nf_conntrack_tcp_timeout_fin_wait",
        sysctl.conntrack.tcp_timeout_fin_wait,
    );
    add_row_opt(
        &mut conntrack,
        "nf_conntrack_tcp_max_retrans",
        sysctl.conntrack.tcp_max_retrans,
    );
    println!("{conntrack}");
    println!();

    let mut socket = Table::new();
    socket.load_preset(UTF8_FULL);
    socket.set_header(vec!["Socket Buffers", "Value"]);
    add_row_opt(&mut socket, "net.core.rmem_max", sysctl.socket_buffer.rmem_max);
    add_row_opt(&mut socket, "net.core.wmem_max", sysctl.socket_buffer.wmem_max);
    add_row_opt(&mut socket, "net.core.rmem_default", sysctl.socket_buffer.rmem_default);
    add_row_opt(&mut socket, "net.core.wmem_default", sysctl.socket_buffer.wmem_default);
    add_row_str(&mut socket, "net.ipv4.tcp_rmem", sysctl.socket_buffer.tcp_rmem);
    add_row_str(&mut socket, "net.ipv4.tcp_wmem", sysctl.socket_buffer.tcp_wmem);
    add_row_opt(&mut socket, "net.ipv4.udp_rmem_min", sysctl.socket_buffer.udp_rmem_min);
    add_row_opt(&mut socket, "net.ipv4.udp_wmem_min", sysctl.socket_buffer.udp_wmem_min);
    println!("{socket}");
    println!();

    let mut tcp = Table::new();
    tcp.load_preset(UTF8_FULL);
    tcp.set_header(vec!["TCP Settings", "Value"]);
    add_row_opt(&mut tcp, "net.core.somaxconn", sysctl.tcp.somaxconn);
    add_row_opt(&mut tcp, "net.ipv4.tcp_max_syn_backlog", sysctl.tcp.max_syn_backlog);
    add_row_opt(&mut tcp, "net.ipv4.tcp_tw_reuse", sysctl.tcp.tw_reuse);
    add_row_opt(&mut tcp, "net.ipv4.tcp_fin_timeout", sysctl.tcp.fin_timeout);
    add_row_opt(&mut tcp, "net.ipv4.tcp_keepalive_time", sysctl.tcp.keepalive_time);
    add_row_opt(&mut tcp, "net.ipv4.tcp_keepalive_probes", sysctl.tcp.keepalive_probes);
    add_row_opt(&mut tcp, "net.ipv4.tcp_keepalive_intvl", sysctl.tcp.keepalive_intvl);
    add_row_str(&mut tcp, "net.ipv4.ip_local_port_range", sysctl.tcp.ip_local_port_range);
    println!("{tcp}");
    println!();

    let mut arp = Table::new();
    arp.load_preset(UTF8_FULL);
    arp.set_header(vec!["ARP / Neighbor Table", "Value"]);
    add_row_opt(&mut arp, "net.ipv4.neigh.default.gc_thresh1", sysctl.arp.gc_thresh1);
    add_row_opt(&mut arp, "net.ipv4.neigh.default.gc_thresh2", sysctl.arp.gc_thresh2);
    add_row_opt(&mut arp, "net.ipv4.neigh.default.gc_thresh3", sysctl.arp.gc_thresh3);
    add_row_opt(&mut arp, "net.ipv4.conf.all.arp_ignore", sysctl.arp.arp_ignore);
    add_row_opt(&mut arp, "net.ipv4.conf.all.arp_announce", sysctl.arp.arp_announce);
    println!("{arp}");
    println!();

    let mut rp = Table::new();
    rp.load_preset(UTF8_FULL);
    rp.set_header(vec!["Reverse Path Filtering", "Value"]);
    add_row_opt(&mut rp, "net.ipv4.conf.all.rp_filter", sysctl.rp_filter.all);
    add_row_opt(&mut rp, "net.ipv4.conf.default.rp_filter", sysctl.rp_filter.default);
    println!("{rp}");
}

fn add_row_opt(table: &mut Table, name: &str, value: Option<u64>) {
    table.add_row(vec![
        name.to_string(),
        value.map(|v| v.to_string()).unwrap_or("-".to_string()),
    ]);
}

fn add_row_str(table: &mut Table, name: &str, value: Option<String>) {
    table.add_row(vec![name.to_string(), value.unwrap_or("-".to_string())]);
}
