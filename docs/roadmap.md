# Roadmap

- [x] Persistent service instances (database storage)
- [x] Distro protocol enhancement (AP mode)
- [x] Config encryption at rest (AES-128-CBC)
- [x] OpenTelemetry integration (OTLP export)
- [x] Multi-datacenter support (locality-aware sync)
- [x] Consul Connect xDS protocol (EDS, CDS, LDS, RDS, ADS; verified with real Envoy); Nacos Istio MCP `ResourceSource` + config-source ADS `ServiceEntry`, Envoy CDS/EDS/LDS/RDS, and config-tagged VirtualService → RDS on 18848 (DestinationRule remains validate-only like upstream)
- [x] AI Integration (MCP Server Registry, A2A Agent Registry)
- [x] Kubernetes sync (bidirectional service sync)
- [x] LDAP authentication
- [x] OAuth2/OIDC authentication (Google, GitHub, Microsoft)
- [x] Distributed locking (Raft-based)
- [x] DNS-based service discovery
- [x] Gray/Beta release API
- [x] Operation audit logs
- [x] Database auto-migration
- [x] Web console UI — see [batata-ui](https://github.com/easynet-cn/batata-ui)
- [ ] Kubernetes Operator
