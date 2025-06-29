#!/bin/bash

# 🔧 Environment-Specific Build Script
# Builds the solochain with appropriate configuration for different environments

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
ENVIRONMENT="${1:-development}"
BUILD_TYPE="${2:-release}"
TARGET_DIR="target"

echo -e "${BLUE}🔧 Solochain Environment-Specific Build${NC}"
echo -e "${YELLOW}Environment: $ENVIRONMENT${NC}"
echo -e "${YELLOW}Build Type: $BUILD_TYPE${NC}"
echo ""

# Validate environment
case "$ENVIRONMENT" in
    "dev"|"development")
        ENVIRONMENT="development"
        FEATURES=""
        ;;
    "local"|"local-testnet")
        ENVIRONMENT="local-testnet"
        FEATURES="--features local-testnet"
        ;;
    "staging")
        ENVIRONMENT="staging"
        FEATURES="--features staging"
        ;;
    "prod"|"production")
        ENVIRONMENT="production"
        FEATURES="--features production"
        ;;
    *)
        echo -e "${RED}❌ Invalid environment: $ENVIRONMENT${NC}"
        echo -e "${YELLOW}Valid options: dev, local, staging, production${NC}"
        exit 1
        ;;
esac

# Validate build type
case "$BUILD_TYPE" in
    "debug"|"dev")
        BUILD_FLAGS=""
        ;;
    "release"|"prod")
        BUILD_FLAGS="--release"
        ;;
    *)
        echo -e "${RED}❌ Invalid build type: $BUILD_TYPE${NC}"
        echo -e "${YELLOW}Valid options: debug, release${NC}"
        exit 1
        ;;
esac

echo -e "${BLUE}📋 Build Configuration Summary${NC}"
echo -e "  Environment: ${GREEN}$ENVIRONMENT${NC}"
echo -e "  Features: ${GREEN}${FEATURES:-none}${NC}"
echo -e "  Build Flags: ${GREEN}${BUILD_FLAGS:-none}${NC}"
echo ""

# Security warnings for production
if [ "$ENVIRONMENT" = "production" ]; then
    echo -e "${RED}⚠️  PRODUCTION BUILD WARNINGS${NC}"
    echo -e "${YELLOW}  - Ensure production keys have been generated and configured${NC}"
    echo -e "${YELLOW}  - Verify SS58 prefix has been registered for production${NC}"
    echo -e "${YELLOW}  - Review all security parameters before deployment${NC}"
    echo -e "${YELLOW}  - Test thoroughly in staging environment first${NC}"
    echo ""
    
    read -p "Continue with production build? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo -e "${YELLOW}Build cancelled.${NC}"
        exit 0
    fi
fi

# Build runtime first
echo -e "${BLUE}🏗️  Building runtime with $ENVIRONMENT configuration...${NC}"
if [ "$BUILD_TYPE" = "release" ]; then
    cargo build --release -p solochain-template-runtime $FEATURES
else
    cargo build -p solochain-template-runtime $FEATURES
fi

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ Runtime build completed successfully${NC}"
else
    echo -e "${RED}❌ Runtime build failed${NC}"
    exit 1
fi

# Build node
echo -e "${BLUE}🏗️  Building node with $ENVIRONMENT configuration...${NC}"
if [ "$BUILD_TYPE" = "release" ]; then
    cargo build --release -p solochain-template-node $FEATURES
else
    cargo build -p solochain-template-node $FEATURES
fi

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ Node build completed successfully${NC}"
else
    echo -e "${RED}❌ Node build failed${NC}"
    exit 1
fi

# Output binary information
BINARY_PATH="$TARGET_DIR/$BUILD_TYPE/solochain-template-node"
if [ -f "$BINARY_PATH" ]; then
    BINARY_SIZE=$(du -h "$BINARY_PATH" | cut -f1)
    echo ""
    echo -e "${GREEN}🎉 Build completed successfully!${NC}"
    echo -e "${BLUE}📁 Binary location: $BINARY_PATH${NC}"
    echo -e "${BLUE}📏 Binary size: $BINARY_SIZE${NC}"
    echo ""
    
    # Environment-specific usage instructions
    case "$ENVIRONMENT" in
        "development")
            echo -e "${BLUE}🚀 Development Usage:${NC}"
            echo -e "${YELLOW}  ./$BINARY_PATH --dev${NC}"
            ;;
        "local-testnet")
            echo -e "${BLUE}🚀 Local Testnet Usage:${NC}"
            echo -e "${YELLOW}  ./$BINARY_PATH --chain local${NC}"
            ;;
        "staging")
            echo -e "${BLUE}🚀 Staging Usage:${NC}"
            echo -e "${YELLOW}  ./$BINARY_PATH --chain staging --validator${NC}"
            echo -e "${YELLOW}  # Note: Insert staging keys before starting${NC}"
            ;;
        "production")
            echo -e "${BLUE}🚀 Production Usage:${NC}"
            echo -e "${YELLOW}  ./$BINARY_PATH --chain production --validator${NC}"
            echo -e "${RED}  ⚠️  CRITICAL: Insert production keys before starting${NC}"
            echo -e "${RED}  ⚠️  CRITICAL: Review all security settings${NC}"
            ;;
    esac
    
    echo ""
    echo -e "${BLUE}📖 For more information:${NC}"
    echo -e "${YELLOW}  - Configuration details: runtime/src/configs/environments.rs${NC}"
    echo -e "${YELLOW}  - Security guide: docs/PRODUCTION_SECURITY_GUIDE.md${NC}"
else
    echo -e "${RED}❌ Binary not found at expected location: $BINARY_PATH${NC}"
    exit 1
fi

# Create environment-specific output directory
OUTPUT_DIR="builds/$ENVIRONMENT-$BUILD_TYPE"
mkdir -p "$OUTPUT_DIR"
cp "$BINARY_PATH" "$OUTPUT_DIR/"

# Generate environment summary
cat > "$OUTPUT_DIR/BUILD_INFO.md" << EOF
# Build Information

**Environment**: $ENVIRONMENT  
**Build Type**: $BUILD_TYPE  
**Build Date**: $(date)  
**Binary**: solochain-template-node  
**Size**: $BINARY_SIZE  

## Configuration Summary

### Rate Limiting Parameters
- Max Transactions Per Block: $([ "$ENVIRONMENT" = "production" ] && echo "100" || [ "$ENVIRONMENT" = "staging" ] && echo "200" || [ "$ENVIRONMENT" = "local-testnet" ] && echo "500" || echo "1000")
- Max Transactions Per Minute: $([ "$ENVIRONMENT" = "production" ] && echo "600" || [ "$ENVIRONMENT" = "staging" ] && echo "1200" || [ "$ENVIRONMENT" = "local-testnet" ] && echo "3000" || echo "6000")
- Max Bytes Per Account: $([ "$ENVIRONMENT" = "production" ] && echo "512KB" || [ "$ENVIRONMENT" = "staging" ] && echo "512KB" || [ "$ENVIRONMENT" = "local-testnet" ] && echo "1MB" || echo "2MB")

### Consensus Parameters
- Max Authorities: $([ "$ENVIRONMENT" = "production" ] && echo "32" || [ "$ENVIRONMENT" = "staging" ] && echo "21" || [ "$ENVIRONMENT" = "local-testnet" ] && echo "5" || echo "1")
- Allow Multiple Blocks Per Slot: $([ "$ENVIRONMENT" = "development" ] && echo "true" || echo "false")
- Block Hash Count: $([ "$ENVIRONMENT" = "production" ] && echo "7200 (1 hour)" || [ "$ENVIRONMENT" = "staging" ] && echo "2400 (20 min)" || [ "$ENVIRONMENT" = "local-testnet" ] && echo "1200 (10 min)" || echo "250 (2 min)")

### Network Parameters
- SS58 Prefix: 42 $([ "$ENVIRONMENT" = "production" ] && echo "(TODO: Register unique prefix)" || echo "(Generic Substrate)")
- Max Consumers: $([ "$ENVIRONMENT" = "production" ] || [ "$ENVIRONMENT" = "staging" ] && echo "32" || echo "16")

## Usage

\`\`\`bash
$(case "$ENVIRONMENT" in
    "development") echo "./solochain-template-node --dev" ;;
    "local-testnet") echo "./solochain-template-node --chain local" ;;
    "staging") echo "./solochain-template-node --chain staging --validator" ;;
    "production") echo "./solochain-template-node --chain production --validator" ;;
esac)
\`\`\`

$(if [ "$ENVIRONMENT" = "production" ]; then
    echo "## 🔐 Production Security Checklist"
    echo ""
    echo "- [ ] Production keys generated and inserted"
    echo "- [ ] Unique SS58 prefix registered and configured"
    echo "- [ ] Security parameters reviewed and approved"
    echo "- [ ] Tested thoroughly in staging environment"
    echo "- [ ] Monitoring and alerting configured"
    echo "- [ ] Backup and recovery procedures in place"
fi)

## Build Command

\`\`\`bash
$0 $ENVIRONMENT $BUILD_TYPE
\`\`\`
EOF

echo -e "${GREEN}📁 Build artifacts saved to: $OUTPUT_DIR${NC}"
echo -e "${GREEN}🎯 Build completed successfully!${NC}"