use libonm::eth;

pub fn run() {
    let sysctl = eth::get_network_sysctl();

    println!("=== Network Sysctl Settings ===");

    println!();
    println!("-- Connection Tracking (conntrack) --");
    print_opt("net.netfilter.nf_conntrack_max", sysctl.conntrack.max);
    print_opt("net.netfilter.nf_conntrack_buckets", sysctl.conntrack.buckets);
    print_opt(
        "net.netfilter.nf_conntrack_tcp_timeout_established",
        sysctl.conntrack.tcp_timeout_established,
    );
    print_opt(
        "net.netfilter.nf_conntrack_tcp_timeout_time_wait",
        sysctl.conntrack.tcp_timeout_time_wait,
    );
    print_opt(
        "net.netfilter.nf_conntrack_tcp_timeout_close_wait",
        sysctl.conntrack.tcp_timeout_close_wait,
    );
    print_opt(
        "net.netfilter.nf_conntrack_tcp_timeout_fin_wait",
        sysctl.conntrack.tcp_timeout_fin_wait,
    );
    print_opt(
        "net.netfilter.nf_conntrack_tcp_max_retrans",
        sysctl.conntrack.tcp_max_retrans,
    );

    println!();
    println!("-- Socket Buffers --");
    print_opt("net.core.rmem_max", sysctl.socket_buffer.rmem_max);
    print_opt("net.core.wmem_max", sysctl.socket_buffer.wmem_max);
    print_opt("net.core.rmem_default", sysctl.socket_buffer.rmem_default);
    print_opt("net.core.wmem_default", sysctl.socket_buffer.wmem_default);
    print_opt_str("net.ipv4.tcp_rmem", sysctl.socket_buffer.tcp_rmem);
    print_opt_str("net.ipv4.tcp_wmem", sysctl.socket_buffer.tcp_wmem);
    print_opt("net.ipv4.udp_rmem_min", sysctl.socket_buffer.udp_rmem_min);
    print_opt("net.ipv4.udp_wmem_min", sysctl.socket_buffer.udp_wmem_min);

    println!();
    println!("-- TCP Settings --");
    print_opt("net.core.somaxconn", sysctl.tcp.somaxconn);
    print_opt("net.ipv4.tcp_max_syn_backlog", sysctl.tcp.max_syn_backlog);
    print_opt("net.ipv4.tcp_tw_reuse", sysctl.tcp.tw_reuse);
    print_opt("net.ipv4.tcp_fin_timeout", sysctl.tcp.fin_timeout);
    print_opt("net.ipv4.tcp_keepalive_time", sysctl.tcp.keepalive_time);
    print_opt("net.ipv4.tcp_keepalive_probes", sysctl.tcp.keepalive_probes);
    print_opt("net.ipv4.tcp_keepalive_intvl", sysctl.tcp.keepalive_intvl);
    print_opt_str("net.ipv4.ip_local_port_range", sysctl.tcp.ip_local_port_range);

    println!();
    println!("-- ARP / Neighbor Table --");
    print_opt("net.ipv4.neigh.default.gc_thresh1", sysctl.arp.gc_thresh1);
    print_opt("net.ipv4.neigh.default.gc_thresh2", sysctl.arp.gc_thresh2);
    print_opt("net.ipv4.neigh.default.gc_thresh3", sysctl.arp.gc_thresh3);
    print_opt("net.ipv4.conf.all.arp_ignore", sysctl.arp.arp_ignore);
    print_opt("net.ipv4.conf.all.arp_announce", sysctl.arp.arp_announce);

    println!();
    println!("-- Reverse Path Filtering --");
    print_opt("net.ipv4.conf.all.rp_filter", sysctl.rp_filter.all);
    print_opt("net.ipv4.conf.default.rp_filter", sysctl.rp_filter.default);
}

fn print_opt(name: &str, value: Option<u64>) {
    println!(
        "  {:<50} {}",
        name,
        value.map(|v| v.to_string()).unwrap_or("-".to_string())
    );
}

fn print_opt_str(name: &str, value: Option<String>) {
    println!("  {:<50} {}", name, value.unwrap_or("-".to_string()));
}
