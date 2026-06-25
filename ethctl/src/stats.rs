use comfy_table::{presets::UTF8_FULL, Cell, Color, Table};
use libonm::eth::{self, EthError};

pub fn run(iface_name: Option<&str>) -> Result<(), EthError> {
    let stats = eth::get_network_stats();

    print_conntrack_stats(&stats.conntrack);
    print_softnet_stats(&stats.softnet);
    print_socket_stats(&stats.sockets);

    if let Some(name) = iface_name {
        print_interface_stats(name)?;
    }

    Ok(())
}

fn print_conntrack_stats(ct: &eth::ConntrackStats) {
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec!["Conntrack Saturation", "Value"]);

    table.add_row(vec![
        "Current Connections".to_string(),
        ct.current.map(|v| v.to_string()).unwrap_or("-".to_string()),
    ]);
    table.add_row(vec![
        "Max Connections".to_string(),
        ct.max.map(|v| v.to_string()).unwrap_or("-".to_string()),
    ]);

    let usage_str = ct
        .usage_percent
        .map(|p| format!("{:.2}%", p))
        .unwrap_or("-".to_string());

    let usage_cell = if let Some(pct) = ct.usage_percent {
        if pct >= 90.0 {
            Cell::new(&usage_str).fg(Color::Red)
        } else if pct >= 70.0 {
            Cell::new(&usage_str).fg(Color::Yellow)
        } else {
            Cell::new(&usage_str).fg(Color::Green)
        }
    } else {
        Cell::new(&usage_str)
    };

    table.add_row(vec![Cell::new("Usage"), usage_cell]);

    println!("{table}");
    println!();
}

fn print_softnet_stats(sn: &eth::SoftnetStats) {
    let mut summary = Table::new();
    summary.load_preset(UTF8_FULL);
    summary.set_header(vec!["Softnet Summary", "Value"]);

    summary.add_row(vec![
        "Total Processed".to_string(),
        sn.total_processed.to_string(),
    ]);

    let dropped_cell = if sn.total_dropped > 0 {
        Cell::new(sn.total_dropped.to_string()).fg(Color::Red)
    } else {
        Cell::new(sn.total_dropped.to_string()).fg(Color::Green)
    };
    summary.add_row(vec![Cell::new("Total Dropped"), dropped_cell]);

    let squeeze_cell = if sn.total_time_squeeze > 0 {
        Cell::new(sn.total_time_squeeze.to_string()).fg(Color::Yellow)
    } else {
        Cell::new(sn.total_time_squeeze.to_string()).fg(Color::Green)
    };
    summary.add_row(vec![Cell::new("Total Time Squeeze"), squeeze_cell]);

    println!("{summary}");
    println!();

    if !sn.cpus.is_empty() {
        let mut cpu_table = Table::new();
        cpu_table.load_preset(UTF8_FULL);
        cpu_table.set_header(vec![
            "CPU",
            "Processed",
            "Dropped",
            "Time Squeeze",
            "CPU Collision",
            "RPS",
            "Flow Limit",
        ]);

        for cpu in &sn.cpus {
            let dropped_cell = if cpu.dropped > 0 {
                Cell::new(cpu.dropped.to_string()).fg(Color::Red)
            } else {
                Cell::new(cpu.dropped.to_string())
            };
            let squeeze_cell = if cpu.time_squeeze > 0 {
                Cell::new(cpu.time_squeeze.to_string()).fg(Color::Yellow)
            } else {
                Cell::new(cpu.time_squeeze.to_string())
            };

            cpu_table.add_row(vec![
                Cell::new(format!("CPU{}", cpu.cpu)),
                Cell::new(cpu.processed.to_string()),
                dropped_cell,
                squeeze_cell,
                Cell::new(cpu.cpu_collision.to_string()),
                Cell::new(cpu.received_rps.to_string()),
                Cell::new(cpu.flow_limit_count.to_string()),
            ]);
        }

        println!("{cpu_table}");
        println!();
    }
}

fn print_socket_stats(sock: &eth::SocketStats) {
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec!["Socket Statistics", "Value"]);

    table.add_row(vec![
        "TCP In Use".to_string(),
        sock.tcp_inuse
            .map(|v| v.to_string())
            .unwrap_or("-".to_string()),
    ]);
    table.add_row(vec![
        "TCP Orphan".to_string(),
        sock.tcp_orphan
            .map(|v| v.to_string())
            .unwrap_or("-".to_string()),
    ]);
    table.add_row(vec![
        "TCP Time-Wait".to_string(),
        sock.tcp_tw
            .map(|v| v.to_string())
            .unwrap_or("-".to_string()),
    ]);
    table.add_row(vec![
        "TCP Allocated".to_string(),
        sock.tcp_alloc
            .map(|v| v.to_string())
            .unwrap_or("-".to_string()),
    ]);
    table.add_row(vec![
        "TCP Memory (pages)".to_string(),
        sock.tcp_mem
            .map(|v| v.to_string())
            .unwrap_or("-".to_string()),
    ]);
    table.add_row(vec![
        "UDP In Use".to_string(),
        sock.udp_inuse
            .map(|v| v.to_string())
            .unwrap_or("-".to_string()),
    ]);
    table.add_row(vec![
        "UDP Memory (pages)".to_string(),
        sock.udp_mem
            .map(|v| v.to_string())
            .unwrap_or("-".to_string()),
    ]);
    table.add_row(vec![
        "RAW In Use".to_string(),
        sock.raw_inuse
            .map(|v| v.to_string())
            .unwrap_or("-".to_string()),
    ]);
    table.add_row(vec![
        "FRAG In Use".to_string(),
        sock.frag_inuse
            .map(|v| v.to_string())
            .unwrap_or("-".to_string()),
    ]);
    table.add_row(vec![
        "FRAG Memory".to_string(),
        sock.frag_memory
            .map(|v| v.to_string())
            .unwrap_or("-".to_string()),
    ]);

    println!("{table}");
    println!();
}

fn print_interface_stats(name: &str) -> Result<(), EthError> {
    let stats = eth::get_interface_stats(name)?;

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![&format!("Interface {} Stats", name), "RX", "TX"]);

    table.add_row(vec![
        "Bytes".to_string(),
        format_bytes(stats.rx_bytes),
        format_bytes(stats.tx_bytes),
    ]);
    table.add_row(vec![
        "Packets".to_string(),
        stats.rx_packets.to_string(),
        stats.tx_packets.to_string(),
    ]);

    let rx_err = if stats.rx_errors > 0 {
        Cell::new(stats.rx_errors.to_string()).fg(Color::Red)
    } else {
        Cell::new(stats.rx_errors.to_string())
    };
    let tx_err = if stats.tx_errors > 0 {
        Cell::new(stats.tx_errors.to_string()).fg(Color::Red)
    } else {
        Cell::new(stats.tx_errors.to_string())
    };
    table.add_row(vec![Cell::new("Errors"), rx_err, tx_err]);

    let rx_drop = if stats.rx_dropped > 0 {
        Cell::new(stats.rx_dropped.to_string()).fg(Color::Red)
    } else {
        Cell::new(stats.rx_dropped.to_string())
    };
    let tx_drop = if stats.tx_dropped > 0 {
        Cell::new(stats.tx_dropped.to_string()).fg(Color::Red)
    } else {
        Cell::new(stats.tx_dropped.to_string())
    };
    table.add_row(vec![Cell::new("Dropped"), rx_drop, tx_drop]);

    table.add_row(vec![
        "FIFO Errors".to_string(),
        stats.rx_fifo.to_string(),
        stats.tx_fifo.to_string(),
    ]);
    table.add_row(vec![
        "Frame/Carrier".to_string(),
        stats.rx_frame.to_string(),
        stats.tx_carrier.to_string(),
    ]);
    table.add_row(vec![
        "Collisions".to_string(),
        "-".to_string(),
        stats.tx_collisions.to_string(),
    ]);

    println!("{table}");

    Ok(())
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
