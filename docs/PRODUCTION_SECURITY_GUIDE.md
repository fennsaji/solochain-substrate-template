# 🔐 Production Security Guide

This guide provides step-by-step instructions for securely deploying the solochain in production environments.

## ⚠️ **CRITICAL SECURITY REQUIREMENTS**

### **1. Secure Key Generation**

**NEVER use the template keys in production!** The current production configuration contains placeholder keys that must be replaced.

#### **Required Actions:**

1. **Generate Cryptographically Secure Validator Keys**
   ```bash
   # Install subkey if not already available
   cargo install --force subkey --git https://github.com/paritytech/substrate

   # Generate MICC (Sr25519) keys for each validator
   subkey generate --scheme sr25519 --network substrate
   # Save the secret phrase securely (HSM recommended)
   # Note the public key and SS58 address

   # Generate GRANDPA (Ed25519) keys for each validator  
   subkey generate --scheme ed25519 --network substrate
   # Save the secret phrase securely (HSM recommended)
   # Note the public key
   ```

2. **Generate Secure Account Keys**
   ```bash
   # Generate treasury account
   subkey generate --scheme sr25519 --network substrate
   
   # Generate operations account
   subkey generate --scheme sr25519 --network substrate
   
   # Generate emergency/governance account (if sudo is needed)
   subkey generate --scheme sr25519 --network substrate
   ```

#### **Hardware Security Module (HSM) Integration**

For maximum security, generate and store keys using HSM:

```bash
# Example with YubiHSM (replace with your HSM solution)
# 1. Generate keys on HSM
# 2. Export public keys only
# 3. Never expose private keys from HSM
```

### **2. Update Production Genesis Configuration**

Edit `runtime/src/genesis_config_presets.rs` and replace the placeholder keys:

```rust
pub fn production_config_genesis() -> Value {
    production_genesis(
        vec![
            // Replace with your actual validator public keys
            (
                MiccId::from_str("YOUR_VALIDATOR_1_SR25519_PUBLIC_KEY").unwrap(),
                GrandpaId::from_str("YOUR_VALIDATOR_1_ED25519_PUBLIC_KEY").unwrap(),
            ),
            (
                MiccId::from_str("YOUR_VALIDATOR_2_SR25519_PUBLIC_KEY").unwrap(), 
                GrandpaId::from_str("YOUR_VALIDATOR_2_ED25519_PUBLIC_KEY").unwrap(),
            ),
            // Add more validators as needed
        ],
        vec![
            // Replace with your actual account addresses
            AccountId::from_str("YOUR_TREASURY_ACCOUNT_SS58").unwrap(),
            AccountId::from_str("YOUR_OPERATIONS_ACCOUNT_SS58").unwrap(),
        ],
        // CRITICAL: No sudo for production (keep as None)
        None,
        // Set appropriate initial allocation
        100 * UNIT,
    )
}
```

### **3. SS58 Prefix Registration**

1. **Register Unique SS58 Prefix**
   - Visit: https://github.com/paritytech/ss58-registry
   - Submit PR to claim a unique prefix (suggested range: 1000-9999)
   - Update `runtime/src/configs/mod.rs`:
   ```rust
   pub const SS58Prefix: u8 = YOUR_REGISTERED_PREFIX;
   ```

2. **Update Chain Specifications**
   - Update SS58 format in all chain specs
   - Ensure consistency across all configurations

### **4. Environment-Specific Deployment**

#### **Staging Environment**
```bash
# Build with staging configuration
cargo build --release

# Run staging node
./target/release/solochain-template-node --chain staging \
    --validator \
    --base-path /var/lib/substrate/staging \
    --port 30333 \
    --rpc-port 9944 \
    --telemetry-url "wss://telemetry.polkadot.io/submit/ 0"
```

#### **Production Environment**
```bash
# Build with production configuration
cargo build --release

# Run production node (example)
./target/release/solochain-template-node --chain production \
    --validator \
    --base-path /var/lib/substrate/production \
    --port 30333 \
    --rpc-port 9944 \
    --rpc-methods=Safe \
    --no-telemetry \
    --pruning archive
```

### **5. Validator Key Management**

#### **Key Insertion for Validators**

Each validator must insert their keys:

```bash
# Insert MICC key
curl -H "Content-Type: application/json" \
    --data '{"jsonrpc":"2.0","method":"author_insertKey","params":["micc","YOUR_MICC_SECRET_PHRASE","YOUR_MICC_PUBLIC_KEY"],"id":1}' \
    http://localhost:9944

# Insert GRANDPA key  
curl -H "Content-Type: application/json" \
    --data '{"jsonrpc":"2.0","method":"author_insertKey","params":["gran","YOUR_GRANDPA_SECRET_PHRASE","YOUR_GRANDPA_PUBLIC_KEY"],"id":1}' \
    http://localhost:9944
```

#### **Key Rotation Procedures**

1. **Planned Rotation**
   - Generate new keys using secure methods
   - Update validator set through governance/sudo
   - Coordinate rotation across all validators
   - Verify network stability

2. **Emergency Rotation**
   - Immediate key replacement if compromise suspected
   - Emergency network halt if necessary
   - Post-incident security audit

### **6. Network Security Hardening**

#### **Firewall Configuration**

```bash
# Example iptables rules for validator nodes
# Allow P2P networking
iptables -A INPUT -p tcp --dport 30333 -j ACCEPT

# Allow RPC (restrict to internal networks only)
iptables -A INPUT -p tcp --dport 9944 -s YOUR_INTERNAL_NETWORK/24 -j ACCEPT
iptables -A INPUT -p tcp --dport 9944 -j DROP

# Block all other incoming traffic
iptables -A INPUT -j DROP
```

#### **DDoS Protection**

- Deploy behind load balancer with DDoS protection
- Use rate limiting at network level
- Monitor for unusual traffic patterns
- Implement fail2ban for additional protection

#### **Monitoring and Alerting**

```bash
# Example Prometheus configuration
# Monitor validator performance
# Alert on missed blocks, equivocations
# Track network health metrics
# Monitor rate limiter effectiveness
```

### **7. Security Verification Checklist**

#### **Pre-Production Checklist**

- [ ] **Keys Generated Securely**: No test/development keys in use
- [ ] **Sudo Disabled**: No sudo pallet or secure sudo key only  
- [ ] **SS58 Prefix Registered**: Unique prefix claimed and configured
- [ ] **Rate Limiting Active**: Spam protection verified functional
- [ ] **Monitoring Deployed**: Comprehensive metrics and alerting
- [ ] **Backups Configured**: Key backup and recovery procedures
- [ ] **Network Security**: Firewall and DDoS protection active
- [ ] **Access Controls**: Limited RPC access, no development endpoints
- [ ] **Documentation**: Emergency procedures and contact information

#### **Security Audit**

Before production deployment:

1. **Internal Security Review**
   - Code review of all configuration changes
   - Testing of key rotation procedures
   - Verification of rate limiting effectiveness

2. **External Security Audit** (Recommended)
   - Independent security assessment
   - Penetration testing
   - Formal verification (if required)

3. **Bug Bounty Program** (Optional)
   - Incentivize security research
   - Responsible disclosure procedures

### **8. Incident Response Procedures**

#### **Security Incident Response**

1. **Detection and Assessment**
   - Monitor for security anomalies
   - Assess impact and severity
   - Document incident timeline

2. **Containment**
   - Isolate affected systems
   - Implement emergency measures
   - Preserve evidence

3. **Recovery**
   - Restore from secure backups
   - Implement security fixes
   - Verify system integrity

4. **Post-Incident**
   - Security audit and improvement
   - Update procedures and documentation
   - Communication to stakeholders

### **9. Emergency Contacts and Procedures**

#### **Emergency Response Team**

- **Security Lead**: [Contact Information]
- **DevOps Lead**: [Contact Information]  
- **Network Administrator**: [Contact Information]
- **Management**: [Contact Information]

#### **Emergency Procedures**

```bash
# Emergency network halt (if sudo enabled)
# Only use in extreme circumstances
curl -H "Content-Type: application/json" \
    --data '{"jsonrpc":"2.0","method":"system_emergencyShutdown","params":[],"id":1}' \
    http://localhost:9944
```

---

## ⚠️ **CRITICAL WARNINGS**

1. **NEVER use development/test keys in production**
2. **ALWAYS verify key generation entropy and security**
3. **DISABLE sudo pallet for production or use secure keys only**
4. **REGISTER unique SS58 prefix before production deployment**  
5. **IMPLEMENT comprehensive monitoring and alerting**
6. **MAINTAIN secure key backup and recovery procedures**
7. **REGULARLY audit and update security procedures**

---

## 📞 **Support and Resources**

- **Substrate Documentation**: https://docs.substrate.io/
- **SS58 Registry**: https://github.com/paritytech/ss58-registry
- **Security Best Practices**: https://docs.substrate.io/deploy/security/
- **Key Generation Tools**: https://docs.substrate.io/reference/command-line-tools/subkey/

For additional security guidance, consult with blockchain security experts and consider professional security audits before production deployment.