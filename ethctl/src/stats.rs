use comfy_table::{presets::UTF8_FULL, Cell, Color, Table};
use libonm::eth::{self, EthError};

use crate::format;

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
        ct.current.map(format::count).unwrap_or("-".to_string()),
    ]);
    table.add_row(vec![
        "Max Connections".to_string(),
        ct.max.map(format::count).unwrap_or("-".to_string()),
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
    table.add_row(vec![
        "Insert Failed".to_string(),
        ct.insert_failed
            .map(format::count)
            .unwrap_or("-".to_string()),
    ]);
    table.add_row(vec![
        "Drops".to_string(),
        ct.drop.map(format::count).unwrap_or("-".to_string()),
    ]);
    table.add_row(vec![
        "Early Drops".to_string(),
        ct.early_drop
            .map(format::count)
            .unwrap_or("-".to_string()),
    ]);

    println!("{table}");
    println!();
}

fn print_softnet_stats(sn: &eth::SoftnetStats) {
    let mut summary = Table::new();
    summary.load_preset(UTF8_FULL);
    summary.set_header(vec!["Softnet Summary", "Value"]);

    summary.add_row(vec![
        "Total Processed".to_string(),
        format::count(sn.total_processed),
    ]);

    let dropped_cell = if sn.total_dropped > 0 {
        Cell::new(format::count(sn.total_dropped)).fg(Color::Red)
    } else {
        Cell::new(format::count(sn.total_dropped)).fg(Color::Green)
    };
    summary.add_row(vec![Cell::new("Total Dropped"), dropped_cell]);

    let squeeze_cell = if sn.total_time_squeeze > 0 {
        Cell::new(format::count(sn.total_time_squeeze)).fg(Color::Yellow)
    } else {
        Cell::new(format::count(sn.total_time_squeeze)).fg(Color::Green)
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
                Cell::new(format::count(cpu.dropped)).fg(Color::Red)
            } else {
                Cell::new(format::count(cpu.dropped))
            };
            let squeeze_cell = if cpu.time_squeeze > 0 {
                Cell::new(format::count(cpu.time_squeeze)).fg(Color::Yellow)
            } else {
                Cell::new(format::count(cpu.time_squeeze))
            };

            cpu_table.add_row(vec![
                Cell::new(format!("CPU{}", cpu.cpu)),
                Cell::new(format::count(cpu.processed)),
                dropped_cell,
                squeeze_cell,
                Cell::new(format::count(cpu.cpu_collision)),
                Cell::new(format::count(cpu.received_rps)),
                Cell::new(format::count(cpu.flow_limit_count)),
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
            .map(format::count)
            .unwrap_or("-".to_string()),
    ]);
    table.add_row(vec![
        "TCP Orphan".to_string(),
        sock.tcp_orphan
            .map(format::count)
            .unwrap_or("-".to_string()),
    ]);
    table.add_row(vec![
        "TCP Time-Wait".to_string(),
        sock.tcp_tw
            .map(format::count)
            .unwrap_or("-".to_string()),
    ]);
    table.add_row(vec![
        "TCP Allocated".to_string(),
        sock.tcp_alloc
            .map(format::count)
            .unwrap_or("-".to_string()),
    ]);
    table.add_row(vec![
        "TCP Memory (pages)".to_string(),
        sock.tcp_mem
            .map(format::count)
            .unwrap_or("-".to_string()),
    ]);
    table.add_row(vec![
        "UDP In Use".to_string(),
        sock.udp_inuse
            .map(format::count)
            .unwrap_or("-".to_string()),
    ]);
    table.add_row(vec![
        "UDP Memory (pages)".to_string(),
        sock.udp_mem
            .map(format::count)
            .unwrap_or("-".to_string()),
    ]);
    table.add_row(vec![
        "RAW In Use".to_string(),
        sock.raw_inuse
            .map(format::count)
            .unwrap_or("-".to_string()),
    ]);
    table.add_row(vec![
        "FRAG In Use".to_string(),
        sock.frag_inuse
            .map(format::count)
            .unwrap_or("-".to_string()),
    ]);
    table.add_row(vec![
        "FRAG Memory".to_string(),
        sock.frag_memory
            .map(format::count)
            .unwrap_or("-".to_string()),
    ]);
    table.add_row(vec![
        "TCP Listen Overflows".to_string(),
        sock.listen_overflows
            .map(format::count)
            .unwrap_or("-".to_string()),
    ]);
    table.add_row(vec![
        "TCP Listen Drops".to_string(),
        sock.listen_drops
            .map(format::count)
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
        format::bytes(stats.rx_bytes),
        format::bytes(stats.tx_bytes),
    ]);
    table.add_row(vec![
        "Packets".to_string(),
        format::count(stats.rx_packets),
        format::count(stats.tx_packets),
    ]);

    let rx_err = if stats.rx_errors > 0 {
        Cell::new(format::count(stats.rx_errors)).fg(Color::Red)
    } else {
        Cell::new(format::count(stats.rx_errors))
    };
    let tx_err = if stats.tx_errors > 0 {
        Cell::new(format::count(stats.tx_errors)).fg(Color::Red)
    } else {
        Cell::new(format::count(stats.tx_errors))
    };
    table.add_row(vec![Cell::new("Errors"), rx_err, tx_err]);

    let rx_drop = if stats.rx_dropped > 0 {
        Cell::new(format::count(stats.rx_dropped)).fg(Color::Red)
    } else {
        Cell::new(format::count(stats.rx_dropped))
    };
    let tx_drop = if stats.tx_dropped > 0 {
        Cell::new(format::count(stats.tx_dropped)).fg(Color::Red)
    } else {
        Cell::new(format::count(stats.tx_dropped))
    };
    table.add_row(vec![Cell::new("Dropped"), rx_drop, tx_drop]);

    table.add_row(vec![
        "FIFO Errors".to_string(),
        format::count(stats.rx_fifo),
        format::count(stats.tx_fifo),
    ]);
    table.add_row(vec![
        "Frame/Carrier".to_string(),
        format::count(stats.rx_frame),
        format::count(stats.tx_carrier),
    ]);
    table.add_row(vec![
        "Collisions".to_string(),
        "-".to_string(),
        format::count(stats.tx_collisions),
    ]);

    println!("{table}");

    Ok(())
}
