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

print_table() {
    column -t -s $'\t'
}

print_table_with_header() {
    local first=true
    while IFS= read -r line; do
        echo "$line"
        if [[ "$first" == true ]]; then
            local len=${#line}
            printf '%s\n' "$(printf '=%.0s' $(seq 1 $len))"
            first=false
        fi
    done < <(column -t -s $'\t')
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
    
    {
        echo -e "Name\tMAC Address\tMTU\tState\tSpeed(Mbps)\tDriver\tPCI Slot"
        
        for iface in $interfaces; do
            local mac mtu state speed driver pci_slot
            
            mac=$(read_file "/sys/class/net/$iface/address")
            mtu=$(read_file "/sys/class/net/$iface/mtu")
            state=$(read_file "/sys/class/net/$iface/operstate")
            speed=$(read_file "/sys/class/net/$iface/speed" 2>/dev/null || echo "-")
            [[ "$speed" == "-1" ]] && speed="-"
            
            if [[ -L "/sys/class/net/$iface/device/driver" ]]; then
                driver=$(basename "$(readlink -f /sys/class/net/$iface/device/driver 2>/dev/null)" 2>/dev/null || echo "-")
            else
                driver="-"
            fi
            
            if [[ -L "/sys/class/net/$iface/device" ]]; then
                pci_slot=$(basename "$(readlink -f /sys/class/net/$iface/device 2>/dev/null)" 2>/dev/null || echo "-")
            else
                pci_slot="-"
            fi
            
            echo -e "$iface\t${mac:-"-"}\t${mtu:-"-"}\t${state:-"-"}\t${speed:-"-"}\t${driver:-"-"}\t${pci_slot:-"-"}"
        done
    } | print_table_with_header
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
    
    print_sysctl_table
    echo
    print_interfaces_table
}

print_sysctl_table() {
    {
        echo -e "${CYAN}Overview:${NC}\t\t"
        echo -e "  Profile\t$PROFILE_NAME\t-"
        
        echo -e "\t\t"
        echo -e "${CYAN}Connection Tracking:${NC}\t\t"
        print_sysctl_row "nf_conntrack_max" "net.netfilter.nf_conntrack_max" "$CONNTRACK_MAX"
        print_sysctl_row "nf_conntrack_buckets" "net.netfilter.nf_conntrack_buckets" "$CONNTRACK_BUCKETS"
        print_sysctl_row "nf_conntrack_tcp_timeout_established" "net.netfilter.nf_conntrack_tcp_timeout_established" "$CONNTRACK_TCP_TIMEOUT_ESTABLISHED"
        print_sysctl_row "nf_conntrack_tcp_timeout_time_wait" "net.netfilter.nf_conntrack_tcp_timeout_time_wait" "$CONNTRACK_TCP_TIMEOUT_TIME_WAIT"
        print_sysctl_row "nf_conntrack_tcp_timeout_close_wait" "net.netfilter.nf_conntrack_tcp_timeout_close_wait" "$CONNTRACK_TCP_TIMEOUT_CLOSE_WAIT"
        print_sysctl_row "nf_conntrack_tcp_timeout_fin_wait" "net.netfilter.nf_conntrack_tcp_timeout_fin_wait" "$CONNTRACK_TCP_TIMEOUT_FIN_WAIT"
        print_sysctl_row "nf_conntrack_tcp_max_retrans" "net.netfilter.nf_conntrack_tcp_max_retrans" "$CONNTRACK_TCP_MAX_RETRANS"
        
        echo -e "\t\t"
        echo -e "${CYAN}Socket Buffers:${NC}\t\t"
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
        
        echo -e "\t\t"
        echo -e "${CYAN}TCP Settings:${NC}\t\t"
        print_sysctl_row "net.core.somaxconn" "net.core.somaxconn" "$SOMAXCONN"
        print_sysctl_row "net.ipv4.tcp_max_syn_backlog" "net.ipv4.tcp_max_syn_backlog" "$TCP_MAX_SYN_BACKLOG"
        print_sysctl_row "net.ipv4.tcp_tw_reuse" "net.ipv4.tcp_tw_reuse" "$TCP_TW_REUSE"
        print_sysctl_row "net.ipv4.tcp_fin_timeout" "net.ipv4.tcp_fin_timeout" "$TCP_FIN_TIMEOUT"
        print_sysctl_row "net.ipv4.tcp_keepalive_time" "net.ipv4.tcp_keepalive_time" "$TCP_KEEPALIVE_TIME"
        print_sysctl_row "net.ipv4.tcp_keepalive_probes" "net.ipv4.tcp_keepalive_probes" "$TCP_KEEPALIVE_PROBES"
        print_sysctl_row "net.ipv4.tcp_keepalive_intvl" "net.ipv4.tcp_keepalive_intvl" "$TCP_KEEPALIVE_INTVL"
        print_sysctl_row_str "net.ipv4.ip_local_port_range" "net.ipv4.ip_local_port_range" "$IP_LOCAL_PORT_RANGE"
        
        echo -e "\t\t"
        echo -e "${CYAN}ARP / Neighbor Table:${NC}\t\t"
        print_sysctl_row "net.ipv4.neigh.default.gc_thresh1" "net.ipv4.neigh.default.gc_thresh1" "$ARP_GC_THRESH1"
        print_sysctl_row "net.ipv4.neigh.default.gc_thresh2" "net.ipv4.neigh.default.gc_thresh2" "$ARP_GC_THRESH2"
        print_sysctl_row "net.ipv4.neigh.default.gc_thresh3" "net.ipv4.neigh.default.gc_thresh3" "$ARP_GC_THRESH3"
        print_sysctl_row "net.ipv4.conf.all.arp_ignore" "net.ipv4.conf.all.arp_ignore" "$ARP_IGNORE"
        print_sysctl_row "net.ipv4.conf.all.arp_announce" "net.ipv4.conf.all.arp_announce" "$ARP_ANNOUNCE"
        
        echo -e "\t\t"
        echo -e "${CYAN}Reverse Path Filtering:${NC}\t\t"
        print_sysctl_row "net.ipv4.conf.all.rp_filter" "net.ipv4.conf.all.rp_filter" "$RP_FILTER"
        print_sysctl_row "net.ipv4.conf.default.rp_filter" "net.ipv4.conf.default.rp_filter" "$RP_FILTER"
    } | print_table
}

print_sysctl_row() {
    local name=$1
    local key=$2
    local suggested=$3
    local current
    current=$(read_sysctl "$key")
    echo -e "  $name\t${current:-"-"}\t$suggested"
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
    echo -e "  $name\t$current_fmt\t$suggested_fmt"
}

print_sysctl_row_str() {
    local name=$1
    local key=$2
    local suggested=$3
    local current
    current=$(read_sysctl "$key")
    echo -e "  $name\t${current:-"-"}\t$suggested"
}

print_interfaces_table() {
    local interfaces
    interfaces=$(ls /sys/class/net 2>/dev/null | grep -v "^lo$" || true)
    
    if [[ -z "$interfaces" ]]; then
        return 0
    fi
    
    print_header "Interfaces"
    {
        echo -e "  Name\tMAC Address\tMTU\tTXQ\tState\tSpeed\tDuplex\tNUMA\tDriver\tType"
        
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
            
            if [[ -L "/sys/class/net/$iface/device/driver" ]]; then
                driver=$(basename "$(readlink -f /sys/class/net/$iface/device/driver 2>/dev/null)" 2>/dev/null || echo "-")
            else
                driver="-"
            fi
            
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
            
            local mtu_str txq_str
            mtu_str="${mtu:-"-"}"
            [[ -n "$mtu" && "$mtu" != "$MTU" ]] && mtu_str="$mtu ($MTU)"
            txq_str="${txq:-"-"}"
            [[ -n "$txq" && "$txq" != "$TXQUEUELEN" ]] && txq_str="$txq ($TXQUEUELEN)"
            
            echo -e "  $iface\t${mac:-"-"}\t$mtu_str\t$txq_str\t${state:-"-"}\t${speed:-"-"}\t${duplex:-"-"}\t${numa:-"-"}\t${driver:-"-"}\t$iface_type"
        done
    } | print_table_with_header
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
    
    qdisc=$(ip -o link show "$name" 2>/dev/null | grep -oP 'qdisc \K\S+' || echo "-")
    group=$(ip -o link show "$name" 2>/dev/null | grep -oP 'group \K\S+' || echo "0")
    
    local ring_rx ring_rx_max ring_tx ring_tx_max coalesce_rx coalesce_tx offload_tso offload_gso offload_gro
    local ethtool_available=false
    
    if command -v ethtool &>/dev/null; then
        ethtool_available=true
        ring_rx=$(ethtool -g "$name" 2>/dev/null | grep -A4 "Current hardware" | grep "RX:" | awk '{print $2}' || echo "-")
        ring_rx_max=$(ethtool -g "$name" 2>/dev/null | grep -A4 "Pre-set maximums" | grep "RX:" | awk '{print $2}' || echo "-")
        ring_tx=$(ethtool -g "$name" 2>/dev/null | grep -A4 "Current hardware" | grep "TX:" | awk '{print $2}' || echo "-")
        ring_tx_max=$(ethtool -g "$name" 2>/dev/null | grep -A4 "Pre-set maximums" | grep "TX:" | awk '{print $2}' || echo "-")
        
        coalesce_rx=$(ethtool -c "$name" 2>/dev/null | grep "rx-usecs:" | awk '{print $2}' || echo "-")
        coalesce_tx=$(ethtool -c "$name" 2>/dev/null | grep "tx-usecs:" | awk '{print $2}' || echo "-")
        
        offload_tso=$(ethtool -k "$name" 2>/dev/null | grep "tcp-segmentation-offload:" | awk '{print $2}' || echo "-")
        offload_gso=$(ethtool -k "$name" 2>/dev/null | grep "generic-segmentation-offload:" | awk '{print $2}' || echo "-")
        offload_gro=$(ethtool -k "$name" 2>/dev/null | grep "generic-receive-offload:" | awk '{print $2}' || echo "-")
    fi
    
    {
        echo -e "${CYAN}Overview:${NC}\t\t"
        echo -e "  Profile\t$PROFILE_NAME\t-"
        
        echo -e "\t\t"
        echo -e "${CYAN}IP Link Settings:${NC}\t\t"
        echo -e "  mtu\t$(format_size "${mtu:-0}")\t$(format_size "$MTU")"
        echo -e "  min_mtu\t$(format_size "${min_mtu:-0}")\t-"
        echo -e "  max_mtu\t$(format_size "${max_mtu:-0}")\t-"
        echo -e "  txqueuelen\t${txq:-"-"}\t$TXQUEUELEN"
        echo -e "  num_tx_queues\t${num_tx:-"-"}\t-"
        echo -e "  num_rx_queues\t${num_rx:-"-"}\t-"
        echo -e "  gso_max_size\t$(format_size "${gso_max_size:-0}")\t$(format_size "$GSO_MAX_SIZE")"
        echo -e "  gso_max_segs\t${gso_max_segs:-"-"}\t$GSO_MAX_SEGS"
        echo -e "  gro_max_size\t$(format_size "${gro_max_size:-0}")\t$(format_size "$GRO_MAX_SIZE")"
        echo -e "  tso_max_size\t$(format_size "${tso_max_size:-0}")\t$(format_size "$TSO_MAX_SIZE")"
        echo -e "  tso_max_segs\t${tso_max_segs:-"-"}\t$TSO_MAX_SEGS"
        echo -e "  qdisc\t${qdisc:-"-"}\t-"
        echo -e "  group\t${group:-"-"}\t0"
        
        echo -e "\t\t"
        echo -e "${CYAN}Ethtool Settings:${NC}\t\t"
        if [[ "$ethtool_available" == true ]]; then
            echo -e "  ring_rx\t${ring_rx:-"-"}\t$RING_RX"
            echo -e "  ring_rx_max\t${ring_rx_max:-"-"}\t-"
            echo -e "  ring_tx\t${ring_tx:-"-"}\t$RING_TX"
            echo -e "  ring_tx_max\t${ring_tx_max:-"-"}\t-"
            echo -e "  coalesce_rx_usecs\t${coalesce_rx:-"-"}\t$COALESCE_RX_USECS"
            echo -e "  coalesce_tx_usecs\t${coalesce_tx:-"-"}\t$COALESCE_TX_USECS"
            echo -e "  offload_tso\t${offload_tso:-"-"}\t$OFFLOAD_TSO"
            echo -e "  offload_gso\t${offload_gso:-"-"}\t$OFFLOAD_GSO"
            echo -e "  offload_gro\t${offload_gro:-"-"}\t$OFFLOAD_GRO"
        else
            echo -e "  (ethtool unavailable)\t\t"
        fi
    } | print_table
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
    
    local found=false
    
    {
        echo -e "Family\tDestination\tGateway\tInterface\tMetric\tProtocol\tScope\tType"
        
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
                
                echo -e "IPv4\t$dest\t$gateway\t$iface\t$metric\t$proto\t$scope\t$rtype"
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
                
                echo -e "IPv6\t$dest\t$gateway\t$iface\t$metric\t$proto\t$scope\t$rtype"
            done < <(ip -6 route show 2>/dev/null || true)
        fi
    } | print_table
    
    [[ "$found" == false ]] && echo "No routes found"
}

# ============================================================================
# NAT Command
# ============================================================================

cmd_nat() {
    if ! command -v nft &>/dev/null; then
        print_error "nft not found (nftables required)"
        return 1
    fi
    
    if ! command -v jq &>/dev/null; then
        print_error "jq not found (required for JSON parsing)"
        return 1
    fi
    
    local json_output
    json_output=$(nft -j list ruleset 2>/dev/null)
    
    if [[ -z "$json_output" ]]; then
        echo "No NAT rules found"
        return 0
    fi
    
    local rules_json
    rules_json=$(echo "$json_output" | jq -c '
        .nftables[]
        | select(.rule != null)
        | .rule
        | select(.table == "nat" or (.expr[]? | (has("masquerade") or has("snat") or has("dnat") or has("redirect"))))
    ' 2>/dev/null)
    
    if [[ -z "$rules_json" ]]; then
        echo "No NAT rules found"
        return 0
    fi
    
    print_header "NAT Rules"
    {
        echo -e "  Chain\tType\tProto\tSource\tDestination\tIn\tOut\tTarget\tPackets\tBytes"
        
        while IFS= read -r rule; do
            [[ -z "$rule" ]] && continue
            
            local chain nat_type proto src dest in_if out_if target packets bytes
            
            chain=$(echo "$rule" | jq -r '.chain // "-"')
            
            nat_type=$(echo "$rule" | jq -r '
                .expr[]
                | if has("masquerade") then "MASQUERADE"
                  elif has("snat") then "SNAT"
                  elif has("dnat") then "DNAT"
                  elif has("redirect") then "REDIRECT"
                  else empty
                  end
            ' 2>/dev/null | head -1)
            [[ -z "$nat_type" ]] && nat_type="-"
            
            target=$(echo "$rule" | jq -r '
                .expr[]
                | if has("snat") then
                    .snat | [.addr // empty, .port // empty] | map(tostring) | join(":")
                  elif has("dnat") then
                    .dnat | [.addr // empty, .port // empty] | map(tostring) | join(":")
                  elif has("redirect") then
                    .redirect | .port // "-"
                  elif has("masquerade") then
                    .masquerade | if .port then (.port | if type == "object" and has("range") then "\(.range[0])-\(.range[1])" else tostring end) else "-" end
                  else empty
                  end
            ' 2>/dev/null | head -1)
            [[ -z "$target" || "$target" == ":" ]] && target="-"
            
            proto=$(echo "$rule" | jq -r '
                .expr[]
                | select(has("match"))
                | .match
                | select(.left.meta.key == "l4proto" or .left.payload.protocol != null)
                | if .left.meta.key == "l4proto" then .right
                  elif .left.payload.protocol then .left.payload.protocol
                  else empty
                  end
            ' 2>/dev/null | head -1)
            [[ -z "$proto" ]] && proto="all"
            
            src=$(echo "$rule" | jq -r '
                .expr[]
                | select(has("match"))
                | .match
                | select(.left.payload.field == "saddr" or .left.payload.field == "sport")
                | if .right | type == "object" and has("prefix") then
                    "\(.right.prefix.addr)/\(.right.prefix.len)"
                  else
                    .right | tostring
                  end
            ' 2>/dev/null | tr '\n' ':' | sed 's/:$//')
            [[ -z "$src" ]] && src="*"
            
            dest=$(echo "$rule" | jq -r '
                .expr[]
                | select(has("match"))
                | .match
                | select(.left.payload.field == "daddr" or .left.payload.field == "dport")
                | if .right | type == "object" and has("prefix") then
                    "\(.right.prefix.addr)/\(.right.prefix.len)"
                  else
                    .right | tostring
                  end
            ' 2>/dev/null | tr '\n' ':' | sed 's/:$//')
            [[ -z "$dest" ]] && dest="*"
            
            in_if=$(echo "$rule" | jq -r '
                .expr[]
                | select(has("match"))
                | .match
                | select(.left.meta.key == "iifname" or .left.meta.key == "iif")
                | .right
            ' 2>/dev/null | head -1)
            [[ -z "$in_if" ]] && in_if="*"
            
            out_if=$(echo "$rule" | jq -r '
                .expr[]
                | select(has("match"))
                | .match
                | select(.left.meta.key == "oifname" or .left.meta.key == "oif")
                | .right
            ' 2>/dev/null | head -1)
            [[ -z "$out_if" ]] && out_if="*"
            
            packets=$(echo "$rule" | jq -r '
                .expr[]
                | select(has("counter"))
                | .counter.packets // 0
            ' 2>/dev/null | head -1)
            [[ -z "$packets" ]] && packets="0"
            
            bytes=$(echo "$rule" | jq -r '
                .expr[]
                | select(has("counter"))
                | .counter.bytes // 0
            ' 2>/dev/null | head -1)
            [[ -z "$bytes" ]] && bytes="0"
            
            echo -e "  $chain\t$nat_type\t$proto\t$src\t$dest\t$in_if\t$out_if\t$target\t$(format_count "$packets")\t$(format_bytes_human "$bytes")"
        done <<< "$rules_json"
    } | print_table_with_header
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
