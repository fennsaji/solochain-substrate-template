# 🔧 Environment-Specific Configuration Guide

This guide explains how to use the environment-specific configuration system for deploying the solochain across different environments with appropriate parameter tuning.

## Overview

The solochain supports four distinct environment configurations:
- **Development**: Permissive settings for local development and testing
- **Local Testnet**: Multi-node testing with moderate restrictions
- **Staging**: Production-like settings for pre-deployment testing
- **Production**: Secure, conservative settings for live deployment

## Environment Configuration Parameters

### Rate Limiting Parameters

| Parameter | Development | Local Testnet | Staging | Production |
|-----------|-------------|---------------|---------|------------|
| Default Transactions/Block | 1000 | 500 | 200 | 100 |
| Default Transactions/Minute | 6000 | 3000 | 1200 | 600 |
| Max Transactions/Account | 1000 | 500 | 200 | 100 |
| Max Bytes/Account | 2MB | 1MB | 512KB | 512KB |
| Max Transactions/Minute | 600 | 300 | 120 | 60 |

### Consensus Parameters

| Parameter | Development | Local Testnet | Staging | Production |
|-----------|-------------|---------------|---------|------------|
| Max Authorities | 10 | 5 | 21 | 32 |
| Allow Multiple Blocks/Slot | true | false | false | false |
| Block Hash Count | 250 (2min) | 1200 (10min) | 2400 (20min) | 7200 (1hr) |

### Network Parameters

| Parameter | Development | Local Testnet | Staging | Production |
|-----------|-------------|---------------|---------|------------|
| SS58 Prefix | 42 (generic) | 42 (generic) | 42 (TODO) | 42 (TODO) |
| Max Consumers | 16 | 16 | 32 | 32 |

## Building for Different Environments

### Using the Build Script (Recommended)

```bash
# Development build (default)
./scripts/build-environment.sh dev debug
./scripts/build-environment.sh development release

# Local testnet build
./scripts/build-environment.sh local debug
./scripts/build-environment.sh local-testnet release

# Staging build
./scripts/build-environment.sh staging debug
./scripts/build-environment.sh staging release

# Production build (with security prompts)
./scripts/build-environment.sh production release
```

### Manual Cargo Commands

```bash
# Development (default - no features)
cargo build --release

# Local testnet
cargo build --release --features local-testnet

# Staging
cargo build --release --features staging

# Production
cargo build --release --features production
```

## Runtime Configuration

The environment-specific parameters are defined in `runtime/src/configs/environments.rs` using Rust's `cfg` attributes:

```rust
// Example: Rate limiting for production
#[cfg(feature = "production")]
pub const RATE_LIMIT_DEFAULT_TXS_PER_BLOCK: u32 = 100;

#[cfg(feature = "staging")]
pub const RATE_LIMIT_DEFAULT_TXS_PER_BLOCK: u32 = 200;

// Development default (no feature flags)
#[cfg(not(any(feature = "production", feature = "staging", feature = "local-testnet")))]
pub const RATE_LIMIT_DEFAULT_TXS_PER_BLOCK: u32 = 1000;
```

## Chain Specifications

Each environment has its own chain specification:

### Development
```bash
./solochain-template-node --dev
# Uses: development_config_genesis()
```

### Local Testnet
```bash
./solochain-template-node --chain local
# Uses: local_config_genesis()
```

### Staging
```bash
./solochain-template-node --chain staging --validator
# Uses: staging_config_genesis()
```

### Production
```bash
./solochain-template-node --chain production --validator
# Uses: production_config_genesis()
```

## Configuration Validation

The system includes built-in validation for each environment:

```rust
use solochain_template_runtime::configs::environments::EnvironmentValidator;

// Validate current environment configuration
if let Err(e) = EnvironmentValidator::validate() {
    eprintln!("Configuration validation failed: {}", e);
}

// Get configuration summary
println!("{}", EnvironmentValidator::get_config_summary());
```

### Production Validation Rules

Production builds automatically validate:
- SS58 prefix is not the generic Substrate prefix (42)
- Transaction limits are appropriately conservative
- Multiple blocks per slot is disabled
- Minimum of 3 authorities for security

## Environment Selection

### At Compile Time (Recommended)

Environment is selected at compile time using Cargo features:

```bash
# Development (default)
cargo build --release

# Production
cargo build --release --features production
```

### Benefits of Compile-Time Selection

1. **Performance**: No runtime overhead for environment detection
2. **Security**: Configuration cannot be changed after compilation
3. **Validation**: Build-time validation ensures correct configuration
4. **Optimization**: Compiler can optimize based on known configuration

## Build Artifacts

The build script creates organized artifacts in `builds/` directory:

```
builds/
├── development-debug/
│   ├── solochain-template-node
│   └── BUILD_INFO.md
├── staging-release/
│   ├── solochain-template-node
│   └── BUILD_INFO.md
└── production-release/
    ├── solochain-template-node
    └── BUILD_INFO.md
```

Each `BUILD_INFO.md` contains:
- Environment configuration summary
- Build metadata
- Usage instructions
- Security checklist (for production)

## Production Deployment Checklist

Before deploying to production:

1. **Build with Production Features**
   ```bash
   ./scripts/build-environment.sh production release
   ```

2. **Review Configuration**
   - Verify rate limiting parameters
   - Confirm consensus settings
   - Check SS58 prefix registration

3. **Security Validation**
   - Generate production keys (never use development keys)
   - Configure unique SS58 prefix
   - Review all security parameters

4. **Testing**
   - Test in staging environment first
   - Validate key insertion procedures
   - Verify monitoring and alerting

## Adding New Environment Parameters

To add new environment-specific parameters:

1. **Define Constants**
   ```rust
   // In runtime/src/configs/environments.rs
   
   #[cfg(feature = "production")]
   pub const NEW_PARAMETER: u32 = 100;
   
   #[cfg(feature = "staging")]
   pub const NEW_PARAMETER: u32 = 200;
   
   #[cfg(not(any(feature = "production", feature = "staging", feature = "local-testnet")))]
   pub const NEW_PARAMETER: u32 = 1000;
   ```

2. **Update Runtime Configuration**
   ```rust
   // In runtime/src/configs/mod.rs
   
   impl SomeConfig for Runtime {
       type SomeParameter = ConstU32<NEW_PARAMETER>;
   }
   ```

3. **Update Validation**
   ```rust
   // In EnvironmentValidator::validate()
   
   if NEW_PARAMETER > some_limit {
       errors.push("Parameter too high for production".to_string());
   }
   ```

## Troubleshooting

### Feature Not Found Error
```
error: the package does not contain this feature: staging
```
**Solution**: Ensure both runtime and node Cargo.toml files include the environment features.

### Configuration Validation Failed
```
Production environment validation failed:
Production SS58 prefix must not be 42
```
**Solution**: Register a unique SS58 prefix and update the configuration.

### Build Script Permissions
```
Permission denied: ./scripts/build-environment.sh
```
**Solution**: Make the script executable:
```bash
chmod +x scripts/build-environment.sh
```

## Best Practices

1. **Always use the build script** for consistent environment builds
2. **Test in staging** before production deployment
3. **Validate configuration** before deployment
4. **Document changes** to environment parameters
5. **Review security settings** for production builds
6. **Use version control** for environment-specific configurations

## Security Considerations

### Development Environment
- Permissive settings for ease of development
- Generic SS58 prefix acceptable
- High transaction limits for testing

### Production Environment
- Conservative security settings
- Unique SS58 prefix required
- Comprehensive validation required
- No sudo access (disabled in genesis)

## Related Documentation

- [Production Security Guide](PRODUCTION_SECURITY_GUIDE.md)
- [Security Audit Findings](AUDIT_FINDINGS.md)
- [Key Generation Guide](../scripts/generate-production-keys.sh)
- [Runtime Configuration](../runtime/src/configs/environments.rs)