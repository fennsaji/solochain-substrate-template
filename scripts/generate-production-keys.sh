#!/bin/bash

# 🔐 Production Key Generation Script
# WARNING: This script generates production keys for the solochain
# Ensure this is run in a secure, air-gapped environment

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
NUM_VALIDATORS=${1:-3}
OUTPUT_DIR="production-keys-$(date +%Y%m%d-%H%M%S)"
NETWORK="solochain"

echo -e "${BLUE}🔐 Solochain Production Key Generation${NC}"
echo -e "${YELLOW}⚠️  WARNING: Ensure this is run in a secure, air-gapped environment${NC}"
echo ""

# Check if subkey is installed
if ! command -v subkey &> /dev/null; then
    echo -e "${RED}❌ subkey not found. Installing...${NC}"
    echo "Installing subkey..."
    cargo install --force subkey --git https://github.com/paritytech/substrate || {
        echo -e "${RED}❌ Failed to install subkey. Please install manually.${NC}"
        exit 1
    }
    echo -e "${GREEN}✅ subkey installed successfully${NC}"
fi

# Create output directory
mkdir -p "$OUTPUT_DIR"
cd "$OUTPUT_DIR"

echo -e "${BLUE}📁 Keys will be generated in: $(pwd)${NC}"
echo ""

# Function to generate validator keys
generate_validator_keys() {
    local validator_id=$1
    local validator_dir="validator-$validator_id"
    
    echo -e "${BLUE}🔑 Generating keys for Validator $validator_id...${NC}"
    
    mkdir -p "$validator_dir"
    
    # Generate MICC (Sr25519) key
    echo -e "${YELLOW}  Generating MICC (Sr25519) key...${NC}"
    subkey generate --scheme sr25519 --network substrate > "$validator_dir/micc-key.txt"
    
    # Generate GRANDPA (Ed25519) key
    echo -e "${YELLOW}  Generating GRANDPA (Ed25519) key...${NC}"
    subkey generate --scheme ed25519 --network substrate > "$validator_dir/grandpa-key.txt"
    
    # Extract public keys
    MICC_PUBLIC=$(grep "Public key (hex):" "$validator_dir/micc-key.txt" | awk '{print $4}')
    GRANDPA_PUBLIC=$(grep "Public key (hex):" "$validator_dir/grandpa-key.txt" | awk '{print $4}')
    MICC_SS58=$(grep "SS58 Address:" "$validator_dir/micc-key.txt" | awk '{print $3}')
    
    # Create summary
    cat > "$validator_dir/summary.txt" << EOF
Validator $validator_id Key Summary
=================================
MICC (Sr25519) Public Key: $MICC_PUBLIC
GRANDPA (Ed25519) Public Key: $GRANDPA_PUBLIC
SS58 Address: $MICC_SS58

⚠️  SECURITY NOTES:
- Store secret phrases in secure, offline storage (HSM recommended)
- Never share secret phrases
- Backup keys securely with multiple copies
- Test key insertion before production deployment
EOF
    
    echo -e "${GREEN}  ✅ Validator $validator_id keys generated${NC}"
    
    # Store for genesis config generation
    echo "$MICC_PUBLIC,$GRANDPA_PUBLIC,$MICC_SS58" >> "../validator-keys.csv"
}

# Function to generate account keys
generate_account_keys() {
    local account_name=$1
    local account_dir="accounts"
    
    echo -e "${BLUE}🏦 Generating $account_name account key...${NC}"
    
    mkdir -p "$account_dir"
    
    # Generate Sr25519 key for account
    subkey generate --scheme sr25519 --network substrate > "$account_dir/$account_name-key.txt"
    
    # Extract information
    ACCOUNT_PUBLIC=$(grep "Public key (hex):" "$account_dir/$account_name-key.txt" | awk '{print $4}')
    ACCOUNT_SS58=$(grep "SS58 Address:" "$account_dir/$account_name-key.txt" | awk '{print $3}')
    
    # Create summary
    cat > "$account_dir/$account_name-summary.txt" << EOF
$account_name Account Summary
============================
Public Key: $ACCOUNT_PUBLIC
SS58 Address: $ACCOUNT_SS58

⚠️  SECURITY NOTES:
- Store secret phrase securely
- This account will have initial token allocation
- Consider multi-sig for treasury operations
EOF
    
    echo -e "${GREEN}  ✅ $account_name account key generated${NC}"
    
    # Store for genesis config
    echo "$account_name,$ACCOUNT_PUBLIC,$ACCOUNT_SS58" >> "account-keys.csv"
}

# Initialize CSV files
echo "micc_public,grandpa_public,ss58_address" > "validator-keys.csv"
echo "account_name,public_key,ss58_address" > "account-keys.csv"

# Generate validator keys
echo -e "${BLUE}🏛️  Generating $NUM_VALIDATORS validator key pairs...${NC}"
for i in $(seq 1 $NUM_VALIDATORS); do
    generate_validator_keys $i
done

echo ""

# Generate account keys
echo -e "${BLUE}🏦 Generating account keys...${NC}"
generate_account_keys "treasury"
generate_account_keys "operations"

echo ""

# Generate genesis configuration template
echo -e "${BLUE}📋 Generating genesis configuration template...${NC}"

cat > "genesis-config-template.rs" << 'EOF'
/// Production genesis configuration with generated keys
/// Replace the production_config_genesis() function in runtime/src/genesis_config_presets.rs
pub fn production_config_genesis() -> Value {
    production_genesis(
        // GENERATED VALIDATOR KEYS - INSERT BELOW
        vec![
EOF

# Add validator keys to template
echo "            // Validator keys (replace the template keys):" >> "genesis-config-template.rs"
tail -n +2 "validator-keys.csv" | while IFS=',' read -r micc_public grandpa_public ss58_address; do
    cat >> "genesis-config-template.rs" << EOF
            (
                MiccId::from_str("$micc_public").expect("Valid MICC public key"),
                GrandpaId::from_str("$grandpa_public").expect("Valid GRANDPA public key"),
            ),
EOF
done

# Add account keys to template
cat >> "genesis-config-template.rs" << 'EOF'
        ],
        // GENERATED ACCOUNT KEYS - INSERT BELOW
        vec![
EOF

echo "            // Account keys (replace the template accounts):" >> "genesis-config-template.rs"
tail -n +2 "account-keys.csv" | while IFS=',' read -r account_name public_key ss58_address; do
    cat >> "genesis-config-template.rs" << EOF
            AccountId::from_str("$ss58_address").expect("Valid SS58 address"), // $account_name
EOF
done

cat >> "genesis-config-template.rs" << 'EOF'
        ],
        // CRITICAL: No sudo key for production (None = disabled)
        None,
        // Initial allocation: 100 UNIT tokens per account
        100 * UNIT,
    )
}
EOF

# Generate key insertion script
echo -e "${BLUE}🔧 Generating key insertion script...${NC}"

cat > "insert-keys.sh" << 'EOF'
#!/bin/bash

# Key insertion script for validators
# Run this on each validator node to insert their keys

set -euo pipefail

VALIDATOR_ID=${1:-1}
RPC_URL=${2:-"http://localhost:9944"}

echo "🔑 Inserting keys for Validator $VALIDATOR_ID"
echo "🌐 RPC URL: $RPC_URL"

# Check if keys exist
if [ ! -f "validator-$VALIDATOR_ID/micc-key.txt" ]; then
    echo "❌ Keys for validator $VALIDATOR_ID not found"
    exit 1
fi

# Extract secret phrases (you'll need to parse these manually)
echo "⚠️  Please manually extract the secret phrases from:"
echo "   validator-$VALIDATOR_ID/micc-key.txt"
echo "   validator-$VALIDATOR_ID/grandpa-key.txt"
echo ""
echo "Then run these commands:"
echo ""

# Get public keys for the commands
MICC_PUBLIC=$(grep "Public key (hex):" "validator-$VALIDATOR_ID/micc-key.txt" | awk '{print $4}')
GRANDPA_PUBLIC=$(grep "Public key (hex):" "validator-$VALIDATOR_ID/grandpa-key.txt" | awk '{print $4}')

echo "# Insert MICC key:"
echo "curl -H \"Content-Type: application/json\" \\"
echo "    --data '{\"jsonrpc\":\"2.0\",\"method\":\"author_insertKey\",\"params\":[\"micc\",\"YOUR_MICC_SECRET_PHRASE\",\"$MICC_PUBLIC\"],\"id\":1}' \\"
echo "    $RPC_URL"
echo ""

echo "# Insert GRANDPA key:"
echo "curl -H \"Content-Type: application/json\" \\"
echo "    --data '{\"jsonrpc\":\"2.0\",\"method\":\"author_insertKey\",\"params\":[\"gran\",\"YOUR_GRANDPA_SECRET_PHRASE\",\"$GRANDPA_PUBLIC\"],\"id\":1}' \\"
echo "    $RPC_URL"
echo ""
echo "⚠️  Replace YOUR_*_SECRET_PHRASE with the actual secret phrases from the key files"

EOF

chmod +x "insert-keys.sh"

# Generate security checklist
cat > "SECURITY_CHECKLIST.md" << 'EOF'
# 🔐 Production Deployment Security Checklist

## ✅ Pre-Deployment Checklist

### Key Management
- [ ] All secret phrases stored securely (HSM recommended)
- [ ] Secret phrases backed up in multiple secure locations
- [ ] Secret phrases never transmitted electronically
- [ ] Public keys verified and cross-checked
- [ ] Key insertion tested on staging environment

### Genesis Configuration
- [ ] genesis-config-template.rs reviewed and keys updated
- [ ] No development/test keys in production configuration
- [ ] Token allocations reviewed and approved
- [ ] Sudo disabled (None value confirmed)

### Network Configuration
- [ ] Unique SS58 prefix registered and configured
- [ ] Chain specification updated with production parameters
- [ ] Network name and chain ID updated for production

### Security Verification
- [ ] Rate limiting enabled and tested
- [ ] Monitoring and alerting configured
- [ ] Firewall rules implemented
- [ ] DDoS protection deployed
- [ ] RPC access restricted to internal networks only

### Operational Readiness
- [ ] Validator nodes prepared and secured
- [ ] Key insertion procedures tested
- [ ] Backup and recovery procedures documented
- [ ] Emergency response procedures defined
- [ ] Support team contacts updated

## ⚠️ Critical Security Reminders

1. **NEVER commit secret phrases to version control**
2. **ALWAYS verify key generation entropy and randomness**
3. **DISABLE all development features for production**
4. **IMPLEMENT comprehensive monitoring before deployment**
5. **TEST all procedures in staging environment first**

## 🚨 Emergency Contacts

- Security Lead: [TO BE FILLED]
- DevOps Lead: [TO BE FILLED]  
- Network Admin: [TO BE FILLED]

EOF

# Create final summary
cat > "README.md" << EOF
# 🔐 Production Keys Generated - $(date)

This directory contains cryptographically secure keys for production deployment.

## ⚠️ CRITICAL SECURITY WARNINGS

1. **IMMEDIATELY secure all secret phrases**
2. **NEVER share or transmit secret phrases electronically**
3. **Store backups in multiple secure, offline locations**
4. **Follow the security checklist before deployment**

## 📁 Generated Files

### Validator Keys
- \`validator-*/\`: Individual validator key pairs and summaries
- \`validator-keys.csv\`: Summary of all validator public keys

### Account Keys  
- \`accounts/\`: Account key pairs for treasury and operations
- \`account-keys.csv\`: Summary of all account public keys

### Configuration Templates
- \`genesis-config-template.rs\`: Update runtime/src/genesis_config_presets.rs
- \`insert-keys.sh\`: Script template for key insertion

### Documentation
- \`SECURITY_CHECKLIST.md\`: Pre-deployment security checklist
- \`README.md\`: This file

## 🚀 Next Steps

1. **Secure Storage**: Move all secret phrases to secure storage (HSM recommended)
2. **Update Code**: Replace template keys in genesis configuration
3. **Register SS58**: Apply for unique SS58 prefix registration
4. **Testing**: Test key insertion and validation in staging environment
5. **Review**: Complete security checklist before production deployment

## 📞 Support

For deployment assistance, refer to docs/PRODUCTION_SECURITY_GUIDE.md
EOF

echo ""
echo -e "${GREEN}🎉 Key generation completed successfully!${NC}"
echo ""
echo -e "${BLUE}📁 Generated files location: $(pwd)${NC}"
echo -e "${BLUE}📋 Next steps:${NC}"
echo -e "${YELLOW}  1. Secure all secret phrases immediately${NC}"
echo -e "${YELLOW}  2. Update genesis configuration with generated keys${NC}"  
echo -e "${YELLOW}  3. Register unique SS58 prefix${NC}"
echo -e "${YELLOW}  4. Follow security checklist before deployment${NC}"
echo ""
echo -e "${RED}⚠️  WARNING: Protect secret phrases - loss = permanent loss of access${NC}"
echo -e "${RED}⚠️  WARNING: Never commit secret phrases to version control${NC}"
echo ""

# Return to original directory
cd ..

echo -e "${GREEN}✅ Production key generation completed${NC}"
echo -e "${BLUE}📁 Keys saved in: $OUTPUT_DIR${NC}"