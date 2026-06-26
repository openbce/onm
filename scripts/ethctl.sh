#!/bin/bash
#
# ethctl.sh - Ethernet command line tool (bash implementation)
# Manages Ethernet interfaces and network sysctl tuning
#
# Usage:
#   ethctl.sh list                          - List all ethernet interfaces
#   ethctl.sh info [-p profile] [-o output] - Show sysctl tuning with suggested values
#   ethctl.sh link -n name [-p profile] [-g output] - Show interface ip link/ethtool settings
#   ethctl.sh route [-4|-6]                 - Show routing table
#   ethctl.sh nat                           - Show NAT rules
#

set -euo pipefail

VERSION="0.1.0"
SCRIPT_NAME="ethctl.sh"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# ============================================================================
# Utility Functions
# ============================================================================

print_error() {
    echo -e "${RED}error:${NC} $1" >&2
}

print_header() {
    echo -e "${CYAN}$1:${NC}"
}

format_size() {
    local bytes=$1
    if [[ $bytes -ge 1073741824 ]]; then
        echo "$((bytes / 1073741824))G"
    elif [[ $bytes -ge 1048576 ]]; then
        echo "$((bytes / 1048576))M"
    elif [[ $bytes -ge 1024 ]]; then
        echo "$((bytes / 1024))K"
    else
        echo "$bytes"
    fi
}

format_count() {
    local count=$1
    if [[ $count -ge 1000000 ]]; then
        printf "%.1fM" "$(echo "scale=1; $count / 1000000" | bc)"
    elif [[ $count -ge 1000 ]]; then
        printf "%.1fK" "$(echo "scale=1; $count / 1000" | bc)"
    else
        echo "$count"
    fi
}

format_bytes_human() {
    local bytes=$1
    if [[ $bytes -ge 1073741824 ]]; then
        printf "%.1fG" "$(echo "scale=1; $bytes / 1073741824" | bc)"
    elif [[ $bytes -ge 1048576 ]]; then
        printf "%.1fM" "$(echo "scale=1; $bytes / 1048576" | bc)"
    elif [[ $bytes -ge 1024 ]]; then
        printf "%.1fK" "$(echo "scale=1; $bytes / 1024" | bc)"
    else
        echo "$bytes"
    fi
}

read_sysctl() {
    local key=$1
    local value
    value=$(sysctl -n "$key" 2>/dev/null || echo "")
    echo "${value:-}"
}

read_file() {
    local path=$1
    if [[ -f "$path" ]]; then
        cat "$path" 2>/dev/null || echo ""
    else
        echo ""
    fi
}

# ============================================================================
# Tuning Profile Values
# ============================================================================

get_profile_values() {
    local profile=$1
    
    case "$profile" in
        control-plane|controlplane|cp|master)
            # Control plane: handles API server, etcd, all node connections
            CONNTRACK_MAX=4194304
            CONNTRACK_BUCKETS=1048576
            CONNTRACK_TCP_TIMEOUT_ESTABLISHED=86400
            CONNTRACK_TCP_TIMEOUT_TIME_WAIT=60
            CONNTRACK_TCP_TIMEOUT_CLOSE_WAIT=60
            CONNTRACK_TCP_TIMEOUT_FIN_WAIT=60
            CONNTRACK_TCP_MAX_RETRANS=3
            RMEM_MAX=268435456
            WMEM_MAX=268435456
            RMEM_DEFAULT=33554432
            WMEM_DEFAULT=33554432
            TCP_RMEM="4096 2097152 268435456"
            TCP_WMEM="4096 2097152 268435456"
            NETDEV_MAX_BACKLOG=50000
            UDP_RMEM_MIN=16384
            UDP_WMEM_MIN=16384
            UDP_MEM="1048576 4194304 16777216"
            SOMAXCONN=65535
            TCP_MAX_SYN_BACKLOG=65535
            TCP_TW_REUSE=1
            TCP_FIN_TIMEOUT=15
            TCP_KEEPALIVE_TIME=600
            TCP_KEEPALIVE_PROBES=3
            TCP_KEEPALIVE_INTVL=15
            IP_LOCAL_PORT_RANGE="1024 65535"
            ARP_GC_THRESH1=16384
            ARP_GC_THRESH2=65536
            ARP_GC_THRESH3=131072
            ARP_IGNORE=1
            ARP_ANNOUNCE=2
            RP_FILTER=0
            TXQUEUELEN=10000
            MTU=9000
            GSO_MAX_SIZE=65536
            GSO_MAX_SEGS=65535
            GRO_MAX_SIZE=65536
            TSO_MAX_SIZE=524280
            TSO_MAX_SEGS=65535
            RING_RX=4096
            RING_TX=4096
            COALESCE_RX_USECS=50
            COALESCE_TX_USECS=50
            OFFLOAD_TSO=on
            OFFLOAD_GSO=on
            OFFLOAD_GRO=on
            PROFILE_NAME="control-plane"
            PROFILE_HEADER="Suggested (CP 10k)"
            ;;
        *)
            # Worker: handles pod traffic, moderate connections
            CONNTRACK_MAX=1048576
            CONNTRACK_BUCKETS=262144
            CONNTRACK_TCP_TIMEOUT_ESTABLISHED=86400
            CONNTRACK_TCP_TIMEOUT_TIME_WAIT=60
            CONNTRACK_TCP_TIMEOUT_CLOSE_WAIT=60
            CONNTRACK_TCP_TIMEOUT_FIN_WAIT=60
            CONNTRACK_TCP_MAX_RETRANS=3
            RMEM_MAX=134217728
            WMEM_MAX=134217728
            RMEM_DEFAULT=16777216
            WMEM_DEFAULT=16777216
            TCP_RMEM="4096 1048576 134217728"
            TCP_WMEM="4096 1048576 134217728"
            NETDEV_MAX_BACKLOG=30000
            UDP_RMEM_MIN=16384
            UDP_WMEM_MIN=16384
            UDP_MEM="786432 2097152 8388608"
            SOMAXCONN=32768
            TCP_MAX_SYN_BACKLOG=32768
            TCP_TW_REUSE=1
            TCP_FIN_TIMEOUT=15
            TCP_KEEPALIVE_TIME=600
            TCP_KEEPALIVE_PROBES=3
            TCP_KEEPALIVE_INTVL=15
            IP_LOCAL_PORT_RANGE="1024 65535"
            ARP_GC_THRESH1=4096
            ARP_GC_THRESH2=8192
            ARP_GC_THRESH3=16384
            ARP_IGNORE=1
            ARP_ANNOUNCE=2
            RP_FILTER=0
            TXQUEUELEN=5000
            MTU=9000
            GSO_MAX_SIZE=65536
            GSO_MAX_SEGS=65535
            GRO_MAX_SIZE=65536
            TSO_MAX_SIZE=262144
            TSO_MAX_SEGS=65535
            RING_RX=2048
            RING_TX=2048
            COALESCE_RX_USECS=100
            COALESCE_TX_USECS=100
            OFFLOAD_TSO=on
            OFFLOAD_GSO=on
            OFFLOAD_GRO=on
            PROFILE_NAME="worker"
            PROFILE_HEADER="Suggested (Worker 10k)"
            ;;
    esac
}

# ============================================================================
# List Command
# ============================================================================

cmd_list() {
    local interfaces
    interfaces=$(ls /sys/class/net 2>/dev/null | grep -v "^lo$" || true)
    
    if [[ -z "$interfaces" ]]; then
        echo "No interfaces found"
        return 0
    fi
    
    printf "%-15s %-18s %-6s %-8s %-12s %-15s %-15s\n" \
        "Name" "MAC Address" "MTU" "State" "Speed(Mbps)" "Driver" "PCI Slot"
    printf "%s\n" "$(printf '=%.0s' {1..100})"
    
    for iface in $interfaces; do
        local mac mtu state speed driver pci_slot
        
        mac=$(read_file "/sys/class/net/$iface/address")
        mtu=$(read_file "/sys/class/net/$iface/mtu")
        state=$(read_file "/sys/class/net/$iface/operstate")
        speed=$(read_file "/sys/class/net/$iface/speed" 2>/dev/null || echo "-")
        [[ "$speed" == "-1" ]] && speed="-"
        
        # Get driver info
        if [[ -L "/sys/class/net/$iface/device/driver" ]]; then
            driver=$(basename "$(readlink -f /sys/class/net/$iface/device/driver 2>/dev/null)" 2>/dev/null || echo "-")
        else
            driver="-"
        fi
        
        # Get PCI slot
        if [[ -L "/sys/class/net/$iface/device" ]]; then
            pci_slot=$(basename "$(readlink -f /sys/class/net/$iface/device 2>/dev/null)" 2>/dev/null || echo "-")
        else
            pci_slot="-"
        fi
        
        printf "%-15s %-18s %-6s %-8s %-12s %-15s %-15s\n" \
            "$iface" "${mac:-"-"}" "${mtu:-"-"}" "${state:-"-"}" "${speed:-"-"}" "${driver:-"-"}" "${pci_slot:-"-"}"
    done
}

# ============================================================================
# Info Command
# ============================================================================

cmd_info() {
    local profile="worker"
    local output=""
    
    while [[ $# -gt 0 ]]; do
        case "$1" in
            -p|--profile)
                profile="$2"
                shift 2
                ;;
            -o|--output)
                output="$2"
                shift 2
                ;;
            *)
                print_error "Unknown option: $1"
                return 1
                ;;
        esac
    done
    
    get_profile_values "$profile"
    
    if [[ -n "$output" ]]; then
        generate_sysctl_output "$output"
        return 0
    fi
    
    # Display sysctl info table
    print_header "Overview"
    printf "  %-45s %-20s %-20s\n" "Profile" "$PROFILE_NAME" "-"
    echo
    
    print_sysctl_table
    echo
    print_interfaces_table
}

print_sysctl_table() {
    print_header "Connection Tracking"
    print_sysctl_row "nf_conntrack_max" "net.netfilter.nf_conntrack_max" "$CONNTRACK_MAX"
    print_sysctl_row "nf_conntrack_buckets" "net.netfilter.nf_conntrack_buckets" "$CONNTRACK_BUCKETS"
    print_sysctl_row "nf_conntrack_tcp_timeout_established" "net.netfilter.nf_conntrack_tcp_timeout_established" "$CONNTRACK_TCP_TIMEOUT_ESTABLISHED"
    print_sysctl_row "nf_conntrack_tcp_timeout_time_wait" "net.netfilter.nf_conntrack_tcp_timeout_time_wait" "$CONNTRACK_TCP_TIMEOUT_TIME_WAIT"
    print_sysctl_row "nf_conntrack_tcp_timeout_close_wait" "net.netfilter.nf_conntrack_tcp_timeout_close_wait" "$CONNTRACK_TCP_TIMEOUT_CLOSE_WAIT"
    print_sysctl_row "nf_conntrack_tcp_timeout_fin_wait" "net.netfilter.nf_conntrack_tcp_timeout_fin_wait" "$CONNTRACK_TCP_TIMEOUT_FIN_WAIT"
    print_sysctl_row "nf_conntrack_tcp_max_retrans" "net.netfilter.nf_conntrack_tcp_max_retrans" "$CONNTRACK_TCP_MAX_RETRANS"
    echo
    
    print_header "Socket Buffers"
    print_sysctl_row_bytes "net.core.rmem_max" "net.core.rmem_max" "$RMEM_MAX"
    print_sysctl_row_bytes "net.core.wmem_max" "net.core.wmem_max" "$WMEM_MAX"
    print_sysctl_row_bytes "net.core.rmem_default" "net.core.rmem_default" "$RMEM_DEFAULT"
    print_sysctl_row_bytes "net.core.wmem_default" "net.core.wmem_default" "$WMEM_DEFAULT"
    print_sysctl_row_str "net.ipv4.tcp_rmem" "net.ipv4.tcp_rmem" "$TCP_RMEM"
    print_sysctl_row_str "net.ipv4.tcp_wmem" "net.ipv4.tcp_wmem" "$TCP_WMEM"
    print_sysctl_row "net.core.netdev_max_backlog" "net.core.netdev_max_backlog" "$NETDEV_MAX_BACKLOG"
    print_sysctl_row_bytes "net.ipv4.udp_rmem_min" "net.ipv4.udp_rmem_min" "$UDP_RMEM_MIN"
    print_sysctl_row_bytes "net.ipv4.udp_wmem_min" "net.ipv4.udp_wmem_min" "$UDP_WMEM_MIN"
    print_sysctl_row_str "net.ipv4.udp_mem" "net.ipv4.udp_mem" "$UDP_MEM"
    echo
    
    print_header "TCP Settings"
    print_sysctl_row "net.core.somaxconn" "net.core.somaxconn" "$SOMAXCONN"
    print_sysctl_row "net.ipv4.tcp_max_syn_backlog" "net.ipv4.tcp_max_syn_backlog" "$TCP_MAX_SYN_BACKLOG"
    print_sysctl_row "net.ipv4.tcp_tw_reuse" "net.ipv4.tcp_tw_reuse" "$TCP_TW_REUSE"
    print_sysctl_row "net.ipv4.tcp_fin_timeout" "net.ipv4.tcp_fin_timeout" "$TCP_FIN_TIMEOUT"
    print_sysctl_row "net.ipv4.tcp_keepalive_time" "net.ipv4.tcp_keepalive_time" "$TCP_KEEPALIVE_TIME"
    print_sysctl_row "net.ipv4.tcp_keepalive_probes" "net.ipv4.tcp_keepalive_probes" "$TCP_KEEPALIVE_PROBES"
    print_sysctl_row "net.ipv4.tcp_keepalive_intvl" "net.ipv4.tcp_keepalive_intvl" "$TCP_KEEPALIVE_INTVL"
    print_sysctl_row_str "net.ipv4.ip_local_port_range" "net.ipv4.ip_local_port_range" "$IP_LOCAL_PORT_RANGE"
    echo
    
    print_header "ARP / Neighbor Table"
    print_sysctl_row "net.ipv4.neigh.default.gc_thresh1" "net.ipv4.neigh.default.gc_thresh1" "$ARP_GC_THRESH1"
    print_sysctl_row "net.ipv4.neigh.default.gc_thresh2" "net.ipv4.neigh.default.gc_thresh2" "$ARP_GC_THRESH2"
    print_sysctl_row "net.ipv4.neigh.default.gc_thresh3" "net.ipv4.neigh.default.gc_thresh3" "$ARP_GC_THRESH3"
    print_sysctl_row "net.ipv4.conf.all.arp_ignore" "net.ipv4.conf.all.arp_ignore" "$ARP_IGNORE"
    print_sysctl_row "net.ipv4.conf.all.arp_announce" "net.ipv4.conf.all.arp_announce" "$ARP_ANNOUNCE"
    echo
    
    print_header "Reverse Path Filtering"
    print_sysctl_row "net.ipv4.conf.all.rp_filter" "net.ipv4.conf.all.rp_filter" "$RP_FILTER"
    print_sysctl_row "net.ipv4.conf.default.rp_filter" "net.ipv4.conf.default.rp_filter" "$RP_FILTER"
}

print_sysctl_row() {
    local name=$1
    local key=$2
    local suggested=$3
    local current
    current=$(read_sysctl "$key")
    printf "  %-45s %-20s %-20s\n" "$name" "${current:-"-"}" "$suggested"
}

print_sysctl_row_bytes() {
    local name=$1
    local key=$2
    local suggested=$3
    local current
    current=$(read_sysctl "$key")
    local current_fmt="${current:-"-"}"
    [[ -n "$current" ]] && current_fmt=$(format_size "$current")
    local suggested_fmt
    suggested_fmt=$(format_size "$suggested")
    printf "  %-45s %-20s %-20s\n" "$name" "$current_fmt" "$suggested_fmt"
}

print_sysctl_row_str() {
    local name=$1
    local key=$2
    local suggested=$3
    local current
    current=$(read_sysctl "$key")
    printf "  %-45s %-20s %-20s\n" "$name" "${current:-"-"}" "$suggested"
}

print_interfaces_table() {
    local interfaces
    interfaces=$(ls /sys/class/net 2>/dev/null | grep -v "^lo$" || true)
    
    if [[ -z "$interfaces" ]]; then
        return 0
    fi
    
    print_header "Interfaces"
    printf "  %-12s %-18s %-10s %-10s %-8s %-8s %-8s %-6s %-12s %-10s\n" \
        "Name" "MAC Address" "MTU" "TXQ" "State" "Speed" "Duplex" "NUMA" "Driver" "Type"
    
    for iface in $interfaces; do
        local mac mtu txq state speed duplex numa driver iface_type
        
        mac=$(read_file "/sys/class/net/$iface/address")
        mtu=$(read_file "/sys/class/net/$iface/mtu")
        txq=$(read_file "/sys/class/net/$iface/tx_queue_len")
        state=$(read_file "/sys/class/net/$iface/operstate")
        speed=$(read_file "/sys/class/net/$iface/speed" 2>/dev/null || echo "-")
        [[ "$speed" == "-1" ]] && speed="-"
        duplex=$(read_file "/sys/class/net/$iface/duplex" 2>/dev/null || echo "-")
        numa=$(read_file "/sys/class/net/$iface/device/numa_node" 2>/dev/null || echo "-")
        [[ "$numa" == "-1" ]] && numa="-"
        
        # Get driver
        if [[ -L "/sys/class/net/$iface/device/driver" ]]; then
            driver=$(basename "$(readlink -f /sys/class/net/$iface/device/driver 2>/dev/null)" 2>/dev/null || echo "-")
        else
            driver="-"
        fi
        
        # Determine interface type
        if [[ -d "/sys/class/net/$iface/bridge" ]]; then
            iface_type="bridge"
        elif [[ -d "/sys/class/net/$iface/bonding" ]]; then
            iface_type="bond"
        elif [[ -f "/sys/class/net/$iface/tun_flags" ]]; then
            iface_type="tun"
        elif [[ "$iface" == veth* ]]; then
            iface_type="veth"
        elif [[ -d "/sys/class/net/$iface/device" ]]; then
            iface_type="physical"
        else
            iface_type="virtual"
        fi
        
        # Format with suggested values
        local mtu_str txq_str
        mtu_str="${mtu:-"-"}"
        [[ -n "$mtu" && "$mtu" != "$MTU" ]] && mtu_str="$mtu ($MTU)"
        txq_str="${txq:-"-"}"
        [[ -n "$txq" && "$txq" != "$TXQUEUELEN" ]] && txq_str="$txq ($TXQUEUELEN)"
        
        printf "  %-12s %-18s %-10s %-10s %-8s %-8s %-8s %-6s %-12s %-10s\n" \
            "$iface" "${mac:-"-"}" "$mtu_str" "$txq_str" "${state:-"-"}" "${speed:-"-"}" "${duplex:-"-"}" "${numa:-"-"}" "${driver:-"-"}" "$iface_type"
    done
}

generate_sysctl_output() {
    local format=$1
    
    case "$format" in
        cmd)
            echo "sysctl -w net.netfilter.nf_conntrack_max=$CONNTRACK_MAX"
            echo "sysctl -w net.netfilter.nf_conntrack_buckets=$CONNTRACK_BUCKETS"
            echo "sysctl -w net.netfilter.nf_conntrack_tcp_timeout_established=$CONNTRACK_TCP_TIMEOUT_ESTABLISHED"
            echo "sysctl -w net.netfilter.nf_conntrack_tcp_timeout_time_wait=$CONNTRACK_TCP_TIMEOUT_TIME_WAIT"
            echo "sysctl -w net.netfilter.nf_conntrack_tcp_timeout_close_wait=$CONNTRACK_TCP_TIMEOUT_CLOSE_WAIT"
            echo "sysctl -w net.netfilter.nf_conntrack_tcp_timeout_fin_wait=$CONNTRACK_TCP_TIMEOUT_FIN_WAIT"
            echo "sysctl -w net.netfilter.nf_conntrack_tcp_max_retrans=$CONNTRACK_TCP_MAX_RETRANS"
            echo "sysctl -w net.core.rmem_max=$RMEM_MAX"
            echo "sysctl -w net.core.wmem_max=$WMEM_MAX"
            echo "sysctl -w net.core.rmem_default=$RMEM_DEFAULT"
            echo "sysctl -w net.core.wmem_default=$WMEM_DEFAULT"
            echo "sysctl -w net.ipv4.tcp_rmem=\"$TCP_RMEM\""
            echo "sysctl -w net.ipv4.tcp_wmem=\"$TCP_WMEM\""
            echo "sysctl -w net.core.netdev_max_backlog=$NETDEV_MAX_BACKLOG"
            echo "sysctl -w net.core.somaxconn=$SOMAXCONN"
            echo "sysctl -w net.ipv4.tcp_max_syn_backlog=$TCP_MAX_SYN_BACKLOG"
            echo "sysctl -w net.ipv4.tcp_tw_reuse=$TCP_TW_REUSE"
            echo "sysctl -w net.ipv4.tcp_fin_timeout=$TCP_FIN_TIMEOUT"
            echo "sysctl -w net.ipv4.tcp_keepalive_time=$TCP_KEEPALIVE_TIME"
            echo "sysctl -w net.ipv4.tcp_keepalive_probes=$TCP_KEEPALIVE_PROBES"
            echo "sysctl -w net.ipv4.tcp_keepalive_intvl=$TCP_KEEPALIVE_INTVL"
            echo "sysctl -w net.ipv4.ip_local_port_range=\"$IP_LOCAL_PORT_RANGE\""
            echo "sysctl -w net.ipv4.udp_rmem_min=$UDP_RMEM_MIN"
            echo "sysctl -w net.ipv4.udp_wmem_min=$UDP_WMEM_MIN"
            echo "sysctl -w net.ipv4.udp_mem=\"$UDP_MEM\""
            echo "sysctl -w net.ipv4.neigh.default.gc_thresh1=$ARP_GC_THRESH1"
            echo "sysctl -w net.ipv4.neigh.default.gc_thresh2=$ARP_GC_THRESH2"
            echo "sysctl -w net.ipv4.neigh.default.gc_thresh3=$ARP_GC_THRESH3"
            echo "sysctl -w net.ipv4.conf.all.arp_ignore=$ARP_IGNORE"
            echo "sysctl -w net.ipv4.conf.all.arp_announce=$ARP_ANNOUNCE"
            echo "sysctl -w net.ipv4.conf.all.rp_filter=$RP_FILTER"
            echo "sysctl -w net.ipv4.conf.default.rp_filter=$RP_FILTER"
            echo
            echo "# Interface tuning (ip link)"
            echo "for iface in \$(ls /sys/class/net | grep -E '^(eth|ens|eno|enp)'); do"
            echo "    ip link set dev \"\$iface\" txqueuelen $TXQUEUELEN"
            echo "    ip link set dev \"\$iface\" mtu $MTU"
            echo "done"
            echo
            echo "# Ethtool tuning (ring buffers, coalesce, offloads)"
            echo "for iface in \$(ls /sys/class/net | grep -E '^(eth|ens|eno|enp)'); do"
            echo "    ethtool -G \"\$iface\" rx $RING_RX tx $RING_TX 2>/dev/null || true"
            echo "    ethtool -C \"\$iface\" rx-usecs $COALESCE_RX_USECS tx-usecs $COALESCE_TX_USECS 2>/dev/null || true"
            echo "    ethtool -K \"\$iface\" tso $OFFLOAD_TSO gso $OFFLOAD_GSO gro $OFFLOAD_GRO 2>/dev/null || true"
            echo "done"
            ;;
        conf|sysctl.conf|file)
            echo "# Sysctl tuning for 10k-node cluster ($PROFILE_NAME profile)"
            echo "# Save to /etc/sysctl.d/99-k8s-tuning.conf and run: sysctl --system"
            echo
            echo "net.netfilter.nf_conntrack_max = $CONNTRACK_MAX"
            echo "net.netfilter.nf_conntrack_buckets = $CONNTRACK_BUCKETS"
            echo "net.netfilter.nf_conntrack_tcp_timeout_established = $CONNTRACK_TCP_TIMEOUT_ESTABLISHED"
            echo "net.netfilter.nf_conntrack_tcp_timeout_time_wait = $CONNTRACK_TCP_TIMEOUT_TIME_WAIT"
            echo "net.netfilter.nf_conntrack_tcp_timeout_close_wait = $CONNTRACK_TCP_TIMEOUT_CLOSE_WAIT"
            echo "net.netfilter.nf_conntrack_tcp_timeout_fin_wait = $CONNTRACK_TCP_TIMEOUT_FIN_WAIT"
            echo "net.netfilter.nf_conntrack_tcp_max_retrans = $CONNTRACK_TCP_MAX_RETRANS"
            echo "net.core.rmem_max = $RMEM_MAX"
            echo "net.core.wmem_max = $WMEM_MAX"
            echo "net.core.rmem_default = $RMEM_DEFAULT"
            echo "net.core.wmem_default = $WMEM_DEFAULT"
            echo "net.ipv4.tcp_rmem = $TCP_RMEM"
            echo "net.ipv4.tcp_wmem = $TCP_WMEM"
            echo "net.core.netdev_max_backlog = $NETDEV_MAX_BACKLOG"
            echo "net.core.somaxconn = $SOMAXCONN"
            echo "net.ipv4.tcp_max_syn_backlog = $TCP_MAX_SYN_BACKLOG"
            echo "net.ipv4.tcp_tw_reuse = $TCP_TW_REUSE"
            echo "net.ipv4.tcp_fin_timeout = $TCP_FIN_TIMEOUT"
            echo "net.ipv4.tcp_keepalive_time = $TCP_KEEPALIVE_TIME"
            echo "net.ipv4.tcp_keepalive_probes = $TCP_KEEPALIVE_PROBES"
            echo "net.ipv4.tcp_keepalive_intvl = $TCP_KEEPALIVE_INTVL"
            echo "net.ipv4.ip_local_port_range = $IP_LOCAL_PORT_RANGE"
            echo "net.ipv4.udp_rmem_min = $UDP_RMEM_MIN"
            echo "net.ipv4.udp_wmem_min = $UDP_WMEM_MIN"
            echo "net.ipv4.udp_mem = $UDP_MEM"
            echo "net.ipv4.neigh.default.gc_thresh1 = $ARP_GC_THRESH1"
            echo "net.ipv4.neigh.default.gc_thresh2 = $ARP_GC_THRESH2"
            echo "net.ipv4.neigh.default.gc_thresh3 = $ARP_GC_THRESH3"
            echo "net.ipv4.conf.all.arp_ignore = $ARP_IGNORE"
            echo "net.ipv4.conf.all.arp_announce = $ARP_ANNOUNCE"
            echo "net.ipv4.conf.all.rp_filter = $RP_FILTER"
            echo "net.ipv4.conf.default.rp_filter = $RP_FILTER"
            echo
            echo "# NOTE: Interface settings (not sysctl) - apply via script or systemd unit:"
            echo "#   ip link set dev <iface> txqueuelen $TXQUEUELEN"
            echo "#   ip link set dev <iface> mtu $MTU (requires network support)"
            echo "#   ethtool -G <iface> rx $RING_RX tx $RING_TX"
            echo "#   ethtool -C <iface> rx-usecs $COALESCE_RX_USECS tx-usecs $COALESCE_TX_USECS"
            echo "#   ethtool -K <iface> tso $OFFLOAD_TSO gso $OFFLOAD_GSO gro $OFFLOAD_GRO"
            ;;
        script|sh|bash)
            echo "#!/bin/bash"
            echo "# Network tuning for 10k-node cluster ($PROFILE_NAME profile)"
            echo "# Run with: sudo bash <script>"
            echo
            echo "set -e"
            echo
            echo "# Sysctl settings"
            echo "sysctl -w net.netfilter.nf_conntrack_max=$CONNTRACK_MAX"
            echo "sysctl -w net.netfilter.nf_conntrack_buckets=$CONNTRACK_BUCKETS"
            echo "sysctl -w net.netfilter.nf_conntrack_tcp_timeout_established=$CONNTRACK_TCP_TIMEOUT_ESTABLISHED"
            echo "sysctl -w net.netfilter.nf_conntrack_tcp_timeout_time_wait=$CONNTRACK_TCP_TIMEOUT_TIME_WAIT"
            echo "sysctl -w net.netfilter.nf_conntrack_tcp_timeout_close_wait=$CONNTRACK_TCP_TIMEOUT_CLOSE_WAIT"
            echo "sysctl -w net.netfilter.nf_conntrack_tcp_timeout_fin_wait=$CONNTRACK_TCP_TIMEOUT_FIN_WAIT"
            echo "sysctl -w net.netfilter.nf_conntrack_tcp_max_retrans=$CONNTRACK_TCP_MAX_RETRANS"
            echo "sysctl -w net.core.rmem_max=$RMEM_MAX"
            echo "sysctl -w net.core.wmem_max=$WMEM_MAX"
            echo "sysctl -w net.core.rmem_default=$RMEM_DEFAULT"
            echo "sysctl -w net.core.wmem_default=$WMEM_DEFAULT"
            echo "sysctl -w net.ipv4.tcp_rmem=\"$TCP_RMEM\""
            echo "sysctl -w net.ipv4.tcp_wmem=\"$TCP_WMEM\""
            echo "sysctl -w net.core.netdev_max_backlog=$NETDEV_MAX_BACKLOG"
            echo "sysctl -w net.core.somaxconn=$SOMAXCONN"
            echo "sysctl -w net.ipv4.tcp_max_syn_backlog=$TCP_MAX_SYN_BACKLOG"
            echo "sysctl -w net.ipv4.tcp_tw_reuse=$TCP_TW_REUSE"
            echo "sysctl -w net.ipv4.tcp_fin_timeout=$TCP_FIN_TIMEOUT"
            echo "sysctl -w net.ipv4.tcp_keepalive_time=$TCP_KEEPALIVE_TIME"
            echo "sysctl -w net.ipv4.tcp_keepalive_probes=$TCP_KEEPALIVE_PROBES"
            echo "sysctl -w net.ipv4.tcp_keepalive_intvl=$TCP_KEEPALIVE_INTVL"
            echo "sysctl -w net.ipv4.ip_local_port_range=\"$IP_LOCAL_PORT_RANGE\""
            echo "sysctl -w net.ipv4.udp_rmem_min=$UDP_RMEM_MIN"
            echo "sysctl -w net.ipv4.udp_wmem_min=$UDP_WMEM_MIN"
            echo "sysctl -w net.ipv4.udp_mem=\"$UDP_MEM\""
            echo "sysctl -w net.ipv4.neigh.default.gc_thresh1=$ARP_GC_THRESH1"
            echo "sysctl -w net.ipv4.neigh.default.gc_thresh2=$ARP_GC_THRESH2"
            echo "sysctl -w net.ipv4.neigh.default.gc_thresh3=$ARP_GC_THRESH3"
            echo "sysctl -w net.ipv4.conf.all.arp_ignore=$ARP_IGNORE"
            echo "sysctl -w net.ipv4.conf.all.arp_announce=$ARP_ANNOUNCE"
            echo "sysctl -w net.ipv4.conf.all.rp_filter=$RP_FILTER"
            echo "sysctl -w net.ipv4.conf.default.rp_filter=$RP_FILTER"
            echo
            echo "# Interface tuning (ip link)"
            echo "for iface in \$(ls /sys/class/net | grep -E '^(eth|ens|eno|enp)'); do"
            echo "    ip link set dev \"\$iface\" txqueuelen $TXQUEUELEN"
            echo "    # MTU $MTU requires network-wide jumbo frame support"
            echo "    # ip link set dev \"\$iface\" mtu $MTU"
            echo "done"
            echo
            echo "# Ethtool tuning (ring buffers, coalesce, offloads)"
            echo "for iface in \$(ls /sys/class/net | grep -E '^(eth|ens|eno|enp)'); do"
            echo "    ethtool -G \"\$iface\" rx $RING_RX tx $RING_TX 2>/dev/null || true"
            echo "    ethtool -C \"\$iface\" rx-usecs $COALESCE_RX_USECS tx-usecs $COALESCE_TX_USECS 2>/dev/null || true"
            echo "    ethtool -K \"\$iface\" tso $OFFLOAD_TSO gso $OFFLOAD_GSO gro $OFFLOAD_GRO 2>/dev/null || true"
            echo "done"
            echo
            echo "echo 'Network tuning applied successfully'"
            ;;
        *)
            print_error "Unknown output format: $format (use: cmd, conf, script)"
            return 1
            ;;
    esac
}

# ============================================================================
# Link Command
# ============================================================================

cmd_link() {
    local name=""
    local profile="worker"
    local generate=""
    
    while [[ $# -gt 0 ]]; do
        case "$1" in
            -n|--name)
                name="$2"
                shift 2
                ;;
            -p|--profile)
                profile="$2"
                shift 2
                ;;
            -g|--generate)
                if [[ -n "${2:-}" && "${2:0:1}" != "-" ]]; then
                    generate="$2"
                    shift 2
                else
                    generate="cmd"
                    shift
                fi
                ;;
            *)
                print_error "Unknown option: $1"
                return 1
                ;;
        esac
    done
    
    if [[ -z "$name" ]]; then
        print_error "Interface name required (-n <name>)"
        return 1
    fi
    
    if [[ ! -d "/sys/class/net/$name" ]]; then
        print_error "Interface '$name' not found"
        return 1
    fi
    
    get_profile_values "$profile"
    
    if [[ -n "$generate" ]]; then
        generate_link_output "$name" "$generate"
        return 0
    fi
    
    print_link_tables "$name"
}

print_link_tables() {
    local name=$1
    
    print_header "Overview"
    printf "  %-25s %-20s %-20s\n" "Profile" "$PROFILE_NAME" "-"
    echo
    
    print_header "IP Link Settings"
    local mtu min_mtu max_mtu txq num_tx num_rx gso_max_size gso_max_segs gro_max_size tso_max_size tso_max_segs qdisc group
    
    mtu=$(read_file "/sys/class/net/$name/mtu")
    min_mtu=$(read_file "/sys/class/net/$name/min_mtu")
    max_mtu=$(read_file "/sys/class/net/$name/max_mtu")
    txq=$(read_file "/sys/class/net/$name/tx_queue_len")
    num_tx=$(read_file "/sys/class/net/$name/queues" 2>/dev/null | grep -c tx- 2>/dev/null || ls -d /sys/class/net/$name/queues/tx-* 2>/dev/null | wc -l || echo "-")
    num_rx=$(read_file "/sys/class/net/$name/queues" 2>/dev/null | grep -c rx- 2>/dev/null || ls -d /sys/class/net/$name/queues/rx-* 2>/dev/null | wc -l || echo "-")
    gso_max_size=$(read_file "/sys/class/net/$name/gso_max_size")
    gso_max_segs=$(read_file "/sys/class/net/$name/gso_max_segs")
    gro_max_size=$(read_file "/sys/class/net/$name/gro_max_size")
    tso_max_size=$(read_file "/sys/class/net/$name/tso_max_size")
    tso_max_segs=$(read_file "/sys/class/net/$name/tso_max_segs")
    
    # Get qdisc from ip link (if available)
    qdisc=$(ip -o link show "$name" 2>/dev/null | grep -oP 'qdisc \K\S+' || echo "-")
    group=$(ip -o link show "$name" 2>/dev/null | grep -oP 'group \K\S+' || echo "0")
    
    printf "  %-25s %-20s %-20s\n" "mtu" "$(format_size "${mtu:-0}")" "$(format_size "$MTU")"
    printf "  %-25s %-20s %-20s\n" "min_mtu" "$(format_size "${min_mtu:-0}")" "-"
    printf "  %-25s %-20s %-20s\n" "max_mtu" "$(format_size "${max_mtu:-0}")" "-"
    printf "  %-25s %-20s %-20s\n" "txqueuelen" "${txq:-"-"}" "$TXQUEUELEN"
    printf "  %-25s %-20s %-20s\n" "num_tx_queues" "${num_tx:-"-"}" "-"
    printf "  %-25s %-20s %-20s\n" "num_rx_queues" "${num_rx:-"-"}" "-"
    printf "  %-25s %-20s %-20s\n" "gso_max_size" "$(format_size "${gso_max_size:-0}")" "$(format_size "$GSO_MAX_SIZE")"
    printf "  %-25s %-20s %-20s\n" "gso_max_segs" "${gso_max_segs:-"-"}" "$GSO_MAX_SEGS"
    printf "  %-25s %-20s %-20s\n" "gro_max_size" "$(format_size "${gro_max_size:-0}")" "$(format_size "$GRO_MAX_SIZE")"
    printf "  %-25s %-20s %-20s\n" "tso_max_size" "$(format_size "${tso_max_size:-0}")" "$(format_size "$TSO_MAX_SIZE")"
    printf "  %-25s %-20s %-20s\n" "tso_max_segs" "${tso_max_segs:-"-"}" "$TSO_MAX_SEGS"
    printf "  %-25s %-20s %-20s\n" "qdisc" "${qdisc:-"-"}" "-"
    printf "  %-25s %-20s %-20s\n" "group" "${group:-"-"}" "0"
    echo
    
    print_header "Ethtool Settings"
    if command -v ethtool &>/dev/null; then
        local ring_rx ring_rx_max ring_tx ring_tx_max coalesce_rx coalesce_tx offload_tso offload_gso offload_gro
        
        # Ring buffer settings
        ring_rx=$(ethtool -g "$name" 2>/dev/null | grep -A4 "Current hardware" | grep "RX:" | awk '{print $2}' || echo "-")
        ring_rx_max=$(ethtool -g "$name" 2>/dev/null | grep -A4 "Pre-set maximums" | grep "RX:" | awk '{print $2}' || echo "-")
        ring_tx=$(ethtool -g "$name" 2>/dev/null | grep -A4 "Current hardware" | grep "TX:" | awk '{print $2}' || echo "-")
        ring_tx_max=$(ethtool -g "$name" 2>/dev/null | grep -A4 "Pre-set maximums" | grep "TX:" | awk '{print $2}' || echo "-")
        
        # Coalesce settings
        coalesce_rx=$(ethtool -c "$name" 2>/dev/null | grep "rx-usecs:" | awk '{print $2}' || echo "-")
        coalesce_tx=$(ethtool -c "$name" 2>/dev/null | grep "tx-usecs:" | awk '{print $2}' || echo "-")
        
        # Offload settings
        offload_tso=$(ethtool -k "$name" 2>/dev/null | grep "tcp-segmentation-offload:" | awk '{print $2}' || echo "-")
        offload_gso=$(ethtool -k "$name" 2>/dev/null | grep "generic-segmentation-offload:" | awk '{print $2}' || echo "-")
        offload_gro=$(ethtool -k "$name" 2>/dev/null | grep "generic-receive-offload:" | awk '{print $2}' || echo "-")
        
        printf "  %-25s %-20s %-20s\n" "ring_rx" "${ring_rx:-"-"}" "$RING_RX"
        printf "  %-25s %-20s %-20s\n" "ring_rx_max" "${ring_rx_max:-"-"}" "-"
        printf "  %-25s %-20s %-20s\n" "ring_tx" "${ring_tx:-"-"}" "$RING_TX"
        printf "  %-25s %-20s %-20s\n" "ring_tx_max" "${ring_tx_max:-"-"}" "-"
        printf "  %-25s %-20s %-20s\n" "coalesce_rx_usecs" "${coalesce_rx:-"-"}" "$COALESCE_RX_USECS"
        printf "  %-25s %-20s %-20s\n" "coalesce_tx_usecs" "${coalesce_tx:-"-"}" "$COALESCE_TX_USECS"
        printf "  %-25s %-20s %-20s\n" "offload_tso" "${offload_tso:-"-"}" "$OFFLOAD_TSO"
        printf "  %-25s %-20s %-20s\n" "offload_gso" "${offload_gso:-"-"}" "$OFFLOAD_GSO"
        printf "  %-25s %-20s %-20s\n" "offload_gro" "${offload_gro:-"-"}" "$OFFLOAD_GRO"
    else
        printf "  %-25s\n" "(ethtool unavailable)"
    fi
}

generate_link_output() {
    local name=$1
    local format=$2
    
    case "$format" in
        cmd)
            echo "ip link set dev $name txqueuelen $TXQUEUELEN"
            echo "ip link set dev $name mtu $MTU"
            echo
            echo "ethtool -G $name rx $RING_RX tx $RING_TX"
            echo "ethtool -C $name rx-usecs $COALESCE_RX_USECS tx-usecs $COALESCE_TX_USECS"
            echo "ethtool -K $name tso $OFFLOAD_TSO gso $OFFLOAD_GSO gro $OFFLOAD_GRO"
            
            # Show current values as comments
            echo
            echo "# Current values for $name:"
            local mtu txq gso_max_size tso_max_size
            mtu=$(read_file "/sys/class/net/$name/mtu")
            txq=$(read_file "/sys/class/net/$name/tx_queue_len")
            gso_max_size=$(read_file "/sys/class/net/$name/gso_max_size")
            tso_max_size=$(read_file "/sys/class/net/$name/tso_max_size")
            [[ -n "$mtu" ]] && echo "#   mtu: $mtu"
            [[ -n "$txq" ]] && echo "#   txqueuelen: $txq"
            [[ -n "$gso_max_size" ]] && echo "#   gso_max_size: $gso_max_size"
            [[ -n "$tso_max_size" ]] && echo "#   tso_max_size: $tso_max_size"
            ;;
        conf|sysctl.conf|file)
            echo "# IP link and ethtool tuning for $name ($PROFILE_NAME profile)"
            echo "# Apply via script or systemd unit"
            echo
            echo "# ip link settings:"
            echo "ip link set dev $name txqueuelen $TXQUEUELEN"
            echo "# ip link set dev $name mtu $MTU (requires network-wide jumbo frame support)"
            echo
            echo "# ethtool settings:"
            echo "ethtool -G $name rx $RING_RX tx $RING_TX"
            echo "ethtool -C $name rx-usecs $COALESCE_RX_USECS tx-usecs $COALESCE_TX_USECS"
            echo "ethtool -K $name tso $OFFLOAD_TSO gso $OFFLOAD_GSO gro $OFFLOAD_GRO"
            ;;
        script|sh|bash)
            echo "#!/bin/bash"
            echo "# IP link and ethtool tuning for $name ($PROFILE_NAME profile)"
            echo "# Run with: sudo bash <script>"
            echo
            echo "set -e"
            echo "IFACE=$name"
            echo
            echo "ip link set dev \"\$IFACE\" txqueuelen $TXQUEUELEN"
            echo "# MTU $MTU requires network-wide jumbo frame support"
            echo "# ip link set dev \"\$IFACE\" mtu $MTU"
            echo
            echo "ethtool -G \"\$IFACE\" rx $RING_RX tx $RING_TX 2>/dev/null || true"
            echo "ethtool -C \"\$IFACE\" rx-usecs $COALESCE_RX_USECS tx-usecs $COALESCE_TX_USECS 2>/dev/null || true"
            echo "ethtool -K \"\$IFACE\" tso $OFFLOAD_TSO gso $OFFLOAD_GSO gro $OFFLOAD_GRO 2>/dev/null || true"
            echo
            echo "echo 'Link tuning for $name applied successfully'"
            ;;
        *)
            print_error "Unknown output format: $format (use: cmd, conf, script)"
            return 1
            ;;
    esac
}

# ============================================================================
# Route Command
# ============================================================================

cmd_route() {
    local ipv4_only=false
    local ipv6_only=false
    
    while [[ $# -gt 0 ]]; do
        case "$1" in
            -4|--ipv4)
                ipv4_only=true
                shift
                ;;
            -6|--ipv6)
                ipv6_only=true
                shift
                ;;
            *)
                print_error "Unknown option: $1"
                return 1
                ;;
        esac
    done
    
    local show_ipv4=true
    local show_ipv6=true
    [[ "$ipv6_only" == true ]] && show_ipv4=false
    [[ "$ipv4_only" == true ]] && show_ipv6=false
    
    printf "%-6s %-25s %-20s %-12s %-8s %-10s %-10s %-10s\n" \
        "Family" "Destination" "Gateway" "Interface" "Metric" "Protocol" "Scope" "Type"
    printf "%s\n" "$(printf '=%.0s' {1..110})"
    
    local found=false
    
    if [[ "$show_ipv4" == true ]]; then
        while IFS= read -r line; do
            [[ -z "$line" ]] && continue
            found=true
            local dest gateway iface metric proto scope rtype
            
            dest=$(echo "$line" | awk '{print $1}')
            [[ "$dest" == "default" ]] && dest="0.0.0.0/0"
            
            gateway=$(echo "$line" | grep -oP 'via \K\S+' || echo "-")
            iface=$(echo "$line" | grep -oP 'dev \K\S+' || echo "-")
            metric=$(echo "$line" | grep -oP 'metric \K\S+' || echo "-")
            proto=$(echo "$line" | grep -oP 'proto \K\S+' || echo "unspec")
            scope=$(echo "$line" | grep -oP 'scope \K\S+' || echo "global")
            rtype=$(echo "$line" | awk '{for(i=1;i<=NF;i++) if($i=="type") print $(i+1)}')
            [[ -z "$rtype" ]] && rtype="unicast"
            
            printf "%-6s %-25s %-20s %-12s %-8s %-10s %-10s %-10s\n" \
                "IPv4" "$dest" "$gateway" "$iface" "$metric" "$proto" "$scope" "$rtype"
        done < <(ip -4 route show 2>/dev/null || true)
    fi
    
    if [[ "$show_ipv6" == true ]]; then
        while IFS= read -r line; do
            [[ -z "$line" ]] && continue
            found=true
            local dest gateway iface metric proto scope rtype
            
            dest=$(echo "$line" | awk '{print $1}')
            [[ "$dest" == "default" ]] && dest="::/0"
            
            gateway=$(echo "$line" | grep -oP 'via \K\S+' || echo "-")
            iface=$(echo "$line" | grep -oP 'dev \K\S+' || echo "-")
            metric=$(echo "$line" | grep -oP 'metric \K\S+' || echo "-")
            proto=$(echo "$line" | grep -oP 'proto \K\S+' || echo "unspec")
            scope=$(echo "$line" | grep -oP 'scope \K\S+' || echo "global")
            rtype=$(echo "$line" | awk '{for(i=1;i<=NF;i++) if($i=="type") print $(i+1)}')
            [[ -z "$rtype" ]] && rtype="unicast"
            
            printf "%-6s %-25s %-20s %-12s %-8s %-10s %-10s %-10s\n" \
                "IPv6" "$dest" "$gateway" "$iface" "$metric" "$proto" "$scope" "$rtype"
        done < <(ip -6 route show 2>/dev/null || true)
    fi
    
    [[ "$found" == false ]] && echo "No routes found"
}

# ============================================================================
# NAT Command
# ============================================================================

cmd_nat() {
    # Check if iptables is available
    if ! command -v iptables &>/dev/null; then
        print_error "iptables not found"
        return 1
    fi
    
    local has_rules=false
    
    printf "%-15s %-12s %-8s %-18s %-18s %-8s %-8s %-20s %-10s %-10s\n" \
        "Chain" "Type" "Protocol" "Source" "Destination" "In" "Out" "Target" "Packets" "Bytes"
    printf "%s\n" "$(printf '=%.0s' {1..140})"
    
    # Parse iptables nat table with verbose output
    while IFS= read -r line; do
        [[ -z "$line" ]] && continue
        [[ "$line" =~ ^Chain ]] && continue
        [[ "$line" =~ ^pkts ]] && continue
        [[ "$line" =~ ^$ ]] && continue
        
        local chain nat_type proto src dest in_if out_if target packets bytes extra
        
        # Parse the line - iptables -L -n -v format:
        # pkts bytes target prot opt in out source destination [extra]
        read -r packets bytes target proto _ in_if out_if src dest extra <<< "$line"
        
        # Skip non-NAT rules
        case "$target" in
            SNAT|DNAT|MASQUERADE) ;;
            *) continue ;;
        esac
        
        has_rules=true
        
        # Determine chain from context (we'll parse chain headers)
        chain="${current_chain:-UNKNOWN}"
        
        # Determine NAT type
        nat_type="$target"
        
        # Format source/destination with port if present
        local src_port dest_port to_target
        src_port=$(echo "$extra" | grep -oP 'spt:\K\d+' || echo "")
        dest_port=$(echo "$extra" | grep -oP 'dpt:\K\d+' || echo "")
        to_target=$(echo "$extra" | grep -oP 'to:\K\S+' || echo "-")
        
        [[ -n "$src_port" ]] && src="$src:$src_port"
        [[ -n "$dest_port" ]] && dest="$dest:$dest_port"
        [[ "$src" == "0.0.0.0/0" ]] && src="*"
        [[ "$dest" == "0.0.0.0/0" ]] && dest="*"
        [[ "$in_if" == "*" ]] && in_if="*"
        [[ "$out_if" == "*" ]] && out_if="*"
        [[ "$proto" == "all" || "$proto" == "0" ]] && proto="all"
        
        # Format target with to: info
        local target_str="$to_target"
        [[ "$nat_type" == "MASQUERADE" ]] && target_str="-"
        
        printf "%-15s %-12s %-8s %-18s %-18s %-8s %-8s %-20s %-10s %-10s\n" \
            "$chain" "$nat_type" "$proto" "$src" "$dest" "$in_if" "$out_if" "$target_str" \
            "$(format_count "$packets")" "$(format_bytes_human "$bytes")"
    done < <(
        for chain in PREROUTING INPUT OUTPUT POSTROUTING; do
            echo "Chain $chain"
            iptables -t nat -L "$chain" -n -v 2>/dev/null | tail -n +3 | while read -r line; do
                current_chain="$chain"
                echo "$line"
            done
        done
    )
    
    [[ "$has_rules" == false ]] && echo "No NAT rules found (SNAT/DNAT/MASQUERADE)"
}

# ============================================================================
# Help
# ============================================================================

show_help() {
    cat << EOF
${SCRIPT_NAME} ${VERSION}
Ethernet command line tool (bash implementation)

USAGE:
    ${SCRIPT_NAME} <COMMAND> [OPTIONS]

COMMANDS:
    list                    List all ethernet interfaces
    info                    Show network tuning info with all interfaces and suggested values
    link                    Show ip link and ethtool settings with suggested values
    route                   Show routing table (IPv4 and IPv6)
    nat                     Show NAT rules (iptables nat table)
    help                    Show this help message

OPTIONS FOR 'info':
    -p, --profile <PROFILE>     Tuning profile: control-plane, worker (default: worker)
    -o, --output <FORMAT>       Output format: cmd, conf, script

OPTIONS FOR 'link':
    -n, --name <INTERFACE>      Interface name (required)
    -p, --profile <PROFILE>     Tuning profile: control-plane, worker (default: worker)
    -g, --generate [FORMAT]     Generate commands: cmd, conf, script (default: cmd)

OPTIONS FOR 'route':
    -4, --ipv4                  Show only IPv4 routes
    -6, --ipv6                  Show only IPv6 routes

EXAMPLES:
    ${SCRIPT_NAME} list
    ${SCRIPT_NAME} info
    ${SCRIPT_NAME} info -p control-plane
    ${SCRIPT_NAME} info -o script > tune-sysctl.sh
    ${SCRIPT_NAME} link -n eth0
    ${SCRIPT_NAME} link -n eth0 -p control-plane -g script
    ${SCRIPT_NAME} route -4
    ${SCRIPT_NAME} nat
EOF
}

# ============================================================================
# Main
# ============================================================================

main() {
    if [[ $# -eq 0 ]]; then
        show_help
        exit 0
    fi
    
    local command=$1
    shift
    
    case "$command" in
        list)
            cmd_list "$@"
            ;;
        info)
            cmd_info "$@"
            ;;
        link)
            cmd_link "$@"
            ;;
        route)
            cmd_route "$@"
            ;;
        nat)
            cmd_nat "$@"
            ;;
        help|--help|-h)
            show_help
            ;;
        --version|-v)
            echo "${SCRIPT_NAME} ${VERSION}"
            ;;
        *)
            print_error "Unknown command: $command"
            echo "Run '${SCRIPT_NAME} help' for usage"
            exit 1
            ;;
    esac
}

main "$@"
