# p2p-raft

A Raft consensus implementation in Rust, with some extra features suitable for use in peer-to-peer applications.

Raft's most well-known use case is in distributed databases and other such distributed services. The choice to distribute the work across a cluster of servers is often made for the purpose of increasing fault-tolerance and availability compared to a single-server solution. However, that cluster is usually under control of a single entity, and can be carefully monitored and managed. Server failure is infrequent, uptime is assumed to be high, and network connectivity is generally good.

Peer-to-peer applications are also distributed, but none of the same assumptions apply. In P2P, every node is under control by a different entity. Some nodes may be on laptops or phones with spotty connectivity or even long periods of downtime. Network conditions are constantly fluctuating, and no node is guaranteed to be connected to any other node at any given time.

The raft algorithm depends on a quorum (majority) of nodes being available at any given time to make decisions together. In a carefully managed server cluster where downtime is rare, this is easy to maintain. In the P2P world, we must have a much more robust solution than standard Raft. p2p-raft attempts to be that solution.