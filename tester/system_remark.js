#!/usr/bin/env node

const { ApiPromise, WsProvider, Keyring } = require('@polkadot/api');
const { program } = require('commander');

// Test accounts with well-known dev keys
const TEST_ACCOUNTS = {
    alice: '//Alice',
    bob: '//Bob',
    charlie: '//Charlie',
    dave: '//Dave',
    eve: '//Eve',
    ferdie: '//Ferdie'
};

// Priority levels for testing
const PRIORITY_LEVELS = {
    low: 1,
    normal: 100,
    high: 1000,
    critical: 10000,
    max: Number.MAX_SAFE_INTEGER
};

class BlockchainTester {
    constructor(endpoint = 'ws://127.0.0.1:9944') {
        this.endpoint = endpoint;
        this.api = null;
        this.keyring = new Keyring({ type: 'sr25519' });
        this.accounts = {};
        this.stats = {
            totalTxs: 0,
            successfulTxs: 0,
            failedTxs: 0,
            startTime: null,
            endTime: null
        };
    }

    async connect() {
        console.log(`🔗 Connecting to ${this.endpoint}...`);
        const wsProvider = new WsProvider(this.endpoint);
        this.api = await ApiPromise.create({ provider: wsProvider });
        
        // Initialize test accounts
        for (const [name, uri] of Object.entries(TEST_ACCOUNTS)) {
            this.accounts[name] = this.keyring.addFromUri(uri);
        }
        
        console.log(`✅ Connected to blockchain`);
        console.log(`📊 Chain: ${await this.api.rpc.system.chain()}`);
        console.log(`🏷️  Version: ${await this.api.rpc.system.version()}`);
    }

    async disconnect() {
        if (this.api) {
            await this.api.disconnect();
            console.log('🔌 Disconnected from blockchain');
        }
    }

    // Single transaction test
    async singleTransaction(account = 'alice', remark = 'Single test transaction', priority = 'normal') {
        console.log(`\n🧪 Single Transaction Test`);
        console.log(`Account: ${account}, Priority: ${priority}, Remark: "${remark}"`);
        
        const signer = this.accounts[account];
        if (!signer) {
            throw new Error(`Unknown account: ${account}`);
        }

        this.stats.startTime = Date.now();
        
        try {
            const hash = await this.sendTransaction(signer, remark, PRIORITY_LEVELS[priority] || PRIORITY_LEVELS.normal);
            console.log(`✅ Transaction successful: ${hash}`);
            this.stats.successfulTxs++;
        } catch (error) {
            console.error(`❌ Transaction failed:`, error.message);
            this.stats.failedTxs++;
        }
        
        this.stats.totalTxs++;
        this.stats.endTime = Date.now();
    }

    // Multiple transactions in series
    async serialTransactions(count = 5, account = 'alice', delay = 100, priority = 'normal') {
        console.log(`\n🔄 Serial Transactions Test`);
        console.log(`Count: ${count}, Account: ${account}, Delay: ${delay}ms, Priority: ${priority}`);
        
        const signer = this.accounts[account];
        if (!signer) {
            throw new Error(`Unknown account: ${account}`);
        }

        // Get initial nonce for this account
        const accountInfo = await this.api.query.system.account(signer.address);
        let currentNonce = accountInfo.nonce.toNumber();

        this.stats.startTime = Date.now();
        
        for (let i = 1; i <= count; i++) {
            try {
                const remark = `Serial transaction ${i}/${count}`;
                console.log(`📤 Sending ${i}/${count} with nonce ${currentNonce}: "${remark}"`);
                
                const hash = await this.sendTransactionWithNonce(signer, remark, PRIORITY_LEVELS[priority] || PRIORITY_LEVELS.normal, currentNonce);
                console.log(`✅ ${i}/${count} successful: ${hash}`);
                this.stats.successfulTxs++;
                currentNonce++;
                
                if (i < count && delay > 0) {
                    console.log(`⏱️  Waiting ${delay}ms...`);
                    await this.sleep(delay);
                }
            } catch (error) {
                console.error(`❌ ${i}/${count} failed:`, error.message);
                this.stats.failedTxs++;
                currentNonce++;
            }
            this.stats.totalTxs++;
        }
        
        this.stats.endTime = Date.now();
    }

    // Multiple transactions in parallel
    async parallelTransactions(count = 5, accounts = ['alice', 'bob', 'charlie'], priority = 'normal') {
        console.log(`\n⚡ Parallel Transactions Test`);
        console.log(`Count per account: ${count}, Accounts: ${accounts.join(', ')}, Priority: ${priority}`);
        
        // Validate accounts
        const signers = accounts.map(name => {
            const signer = this.accounts[name];
            if (!signer) {
                throw new Error(`Unknown account: ${name}`);
            }
            return { name, signer };
        });

        this.stats.startTime = Date.now();
        const promises = [];
        
        // Create promises for all transactions
        for (const { name, signer } of signers) {
            console.log(`📤 Preparing ${count} transactions for ${name}...`);
            
            // Process transactions in parallel for this account with manual nonce management
            const processAccountParallel = async () => {
                const results = [];
                
                // Get current nonce for this account
                const accountInfo = await this.api.query.system.account(signer.address);
                let currentNonce = accountInfo.nonce.toNumber();
                
                // Create all transactions for this account in parallel
                const accountPromises = [];
                for (let i = 1; i <= count; i++) {
                    const remark = `Parallel tx from ${name} #${i}`;
                    const txNonce = currentNonce + i - 1;
                    
                    console.log(`📤 Preparing ${name} #${i} with nonce ${txNonce}: "${remark}"`);
                    
                    const promise = this.sendTransactionWithNonce(signer, remark, PRIORITY_LEVELS[priority] || PRIORITY_LEVELS.normal, txNonce)
                        .then(hash => {
                            console.log(`✅ ${name} #${i} successful: ${hash}`);
                            this.stats.successfulTxs++;
                            results.push({ success: true, account: name, hash });
                        })
                        .catch(error => {
                            console.error(`❌ ${name} #${i} failed:`, error.message);
                            this.stats.failedTxs++;
                            results.push({ success: false, account: name, error: error.message });
                        });
                    
                    accountPromises.push(promise);
                    this.stats.totalTxs++;
                }
                
                // Wait for all transactions from this account to complete
                await Promise.allSettled(accountPromises);
                return results;
            };
            
            promises.push(processAccountParallel());
        }
        
        console.log(`🚀 Launching ${promises.length} parallel transactions...`);
        const results = await Promise.allSettled(promises);
        
        this.stats.endTime = Date.now();
        console.log(`📊 Parallel test completed. Results summary:`);
        
        // Group results by account
        const resultsByAccount = {};
        results.forEach((result) => {
            if (result.status === 'fulfilled') {
                result.value.forEach((tx) => {
                    const { account } = tx;
                    if (!resultsByAccount[account]) {
                        resultsByAccount[account] = { successful: 0, failed: 0 };
                    }
                    if (tx.success) {
                        resultsByAccount[account].successful++;
                    } else {
                        resultsByAccount[account].failed++;
                    }
                });
            }
        });
        
        for (const [account, stats] of Object.entries(resultsByAccount)) {
            console.log(`   ${account}: ${stats.successful} ✅, ${stats.failed} ❌`);
        }
    }

    // Mixed priority test
    async priorityTest(totalTxs = 10) {
        console.log(`\n🏆 Priority Test`);
        console.log(`Total transactions: ${totalTxs} with mixed priorities`);
        
        const priorities = Object.keys(PRIORITY_LEVELS);
        const accounts = Object.keys(this.accounts).slice(0, 3); // Use first 3 accounts
        
        // Get initial nonces for all accounts
        const accountNonces = {};
        for (const account of accounts) {
            const signer = this.accounts[account];
            const accountInfo = await this.api.query.system.account(signer.address);
            accountNonces[account] = accountInfo.nonce.toNumber();
        }
        
        this.stats.startTime = Date.now();
        const promises = [];
        
        for (let i = 1; i <= totalTxs; i++) {
            const priority = priorities[i % priorities.length];
            const account = accounts[i % accounts.length];
            const signer = this.accounts[account];
            const nonce = accountNonces[account]++;
            const remark = `Priority test ${i}/${totalTxs} - ${priority} priority from ${account}`;
            
            const promise = this.sendTransactionWithNonce(signer, remark, PRIORITY_LEVELS[priority], nonce)
                .then(hash => {
                    console.log(`✅ ${priority.toUpperCase()} priority tx ${i} (${account}): ${hash}`);
                    this.stats.successfulTxs++;
                    return { success: true, priority, account };
                })
                .catch(error => {
                    console.error(`❌ ${priority.toUpperCase()} priority tx ${i} (${account}) failed:`, error.message);
                    this.stats.failedTxs++;
                    return { success: false, priority, account };
                });
                
            promises.push(promise);
            this.stats.totalTxs++;
            
            // Add small random delay for more realistic testing
            await this.sleep(Math.random() * 100);
        }
        
        console.log(`🚀 Launching ${totalTxs} mixed priority transactions...`);
        await Promise.allSettled(promises);
        
        this.stats.endTime = Date.now();
    }

    // Stress test
    async stressTest(duration = 30000, interval = 100) {
        console.log(`\n💥 Stress Test`);
        console.log(`Duration: ${duration}ms, Interval: ${interval}ms`);
        
        const endTime = Date.now() + duration;
        const accounts = Object.keys(this.accounts);
        let txCounter = 0;
        
        // Get initial nonces for all accounts
        const accountNonces = {};
        for (const account of accounts) {
            const signer = this.accounts[account];
            const accountInfo = await this.api.query.system.account(signer.address);
            accountNonces[account] = accountInfo.nonce.toNumber();
        }
        
        this.stats.startTime = Date.now();
        
        const promises = [];
        
        while (Date.now() < endTime) {
            txCounter++;
            const account = accounts[txCounter % accounts.length];
            const signer = this.accounts[account];
            const nonce = accountNonces[account]++;
            const remark = `Stress test tx #${txCounter} from ${account}`;
            const priority = Math.random() > 0.8 ? PRIORITY_LEVELS.high : PRIORITY_LEVELS.normal;
            
            const promise = this.sendTransactionWithNonce(signer, remark, priority, nonce)
                .then(hash => {
                    console.log(`✅ Stress tx #${txCounter} (${account}): ${hash}`);
                    this.stats.successfulTxs++;
                })
                .catch(error => {
                    console.error(`❌ Stress tx #${txCounter} (${account}) failed:`, error.message);
                    this.stats.failedTxs++;
                });
                
            promises.push(promise);
            this.stats.totalTxs++;
            
            await this.sleep(interval);
        }
        
        console.log(`⏳ Waiting for all stress test transactions to complete...`);
        await Promise.allSettled(promises);
        
        this.stats.endTime = Date.now();
    }

    // Burst test - send many transactions at once
    async burstTest(burstSize = 20, burstCount = 3, burstInterval = 5000) {
        console.log(`\n💨 Burst Test`);
        console.log(`Burst size: ${burstSize}, Burst count: ${burstCount}, Interval: ${burstInterval}ms`);
        
        this.stats.startTime = Date.now();
        
        for (let burst = 1; burst <= burstCount; burst++) {
            console.log(`\n🎯 Burst ${burst}/${burstCount} - sending ${burstSize} transactions...`);
            
            const accounts = Object.keys(this.accounts);
            
            // Get initial nonces for all accounts for this burst
            const accountNonces = {};
            for (const account of accounts) {
                const signer = this.accounts[account];
                const accountInfo = await this.api.query.system.account(signer.address);
                accountNonces[account] = accountInfo.nonce.toNumber();
            }
            
            const promises = [];
            for (let i = 1; i <= burstSize; i++) {
                const account = accounts[i % accounts.length];
                const signer = this.accounts[account];
                const nonce = accountNonces[account]++;
                const remark = `Burst ${burst} tx ${i}/${burstSize} from ${account}`;
                
                const promise = this.sendTransactionWithNonce(signer, remark, PRIORITY_LEVELS.normal, nonce)
                    .then(hash => {
                        console.log(`✅ Burst ${burst} tx ${i} (${account}): ${hash}`);
                        this.stats.successfulTxs++;
                    })
                    .catch(error => {
                        console.error(`❌ Burst ${burst} tx ${i} (${account}) failed:`, error.message);
                        this.stats.failedTxs++;
                    });
                    
                promises.push(promise);
                this.stats.totalTxs++;
            }
            
            await Promise.allSettled(promises);
            
            if (burst < burstCount) {
                console.log(`⏱️  Waiting ${burstInterval}ms before next burst...`);
                await this.sleep(burstInterval);
            }
        }
        
        this.stats.endTime = Date.now();
    }

    // Helper method to send a transaction
    async sendTransaction(signer, remark, priority = PRIORITY_LEVELS.normal) {
        return new Promise((resolve, reject) => {
            // Create transaction with priority tip
            const tx = this.api.tx.system.remark(remark);
            
            tx.signAndSend(signer, { tip: priority }, ({ status, dispatchError }) => {
                if (status.isInBlock) {
                    resolve(status.asInBlock.toString());
                }
                
                if (dispatchError) {
                    if (dispatchError.isModule) {
                        const decoded = this.api.registry.findMetaError(dispatchError.asModule);
                        const { docs, name, section } = decoded;
                        reject(new Error(`${section}.${name}: ${docs.join(' ')}`));
                    } else {
                        reject(new Error(dispatchError.toString()));
                    }
                }
            }).catch(reject);
        });
    }

    // Helper method to send a transaction with explicit nonce
    async sendTransactionWithNonce(signer, remark, priority = PRIORITY_LEVELS.normal, nonce) {
        return new Promise((resolve, reject) => {
            // Create transaction with priority tip and explicit nonce
            const tx = this.api.tx.system.remark(remark);
            
            tx.signAndSend(signer, { tip: priority, nonce }, ({ status, dispatchError }) => {
                if (status.isInBlock) {
                    resolve(status.asInBlock.toString());
                }
                
                if (dispatchError) {
                    if (dispatchError.isModule) {
                        const decoded = this.api.registry.findMetaError(dispatchError.asModule);
                        const { docs, name, section } = decoded;
                        reject(new Error(`${section}.${name}: ${docs.join(' ')}`));
                    } else {
                        reject(new Error(dispatchError.toString()));
                    }
                }
            }).catch(reject);
        });
    }

    // Helper method for delays
    sleep(ms) {
        return new Promise(resolve => setTimeout(resolve, ms));
    }

    // Print test statistics
    printStats() {
        console.log(`\n📊 Test Statistics:`);
        console.log(`   Total transactions: ${this.stats.totalTxs}`);
        console.log(`   Successful: ${this.stats.successfulTxs} ✅`);
        console.log(`   Failed: ${this.stats.failedTxs} ❌`);
        console.log(`   Success rate: ${((this.stats.successfulTxs / this.stats.totalTxs) * 100).toFixed(1)}%`);
        
        if (this.stats.startTime && this.stats.endTime) {
            const duration = this.stats.endTime - this.stats.startTime;
            const tps = (this.stats.totalTxs / duration * 1000).toFixed(2);
            console.log(`   Duration: ${duration}ms`);
            console.log(`   TPS: ${tps} transactions/second`);
        }
    }
}

// CLI Configuration
program
    .name('blockchain-tester')
    .description('Comprehensive blockchain testing tool')
    .version('1.0.0')
    .option('-e, --endpoint <url>', 'Blockchain endpoint', 'ws://127.0.0.1:9944');

program
    .command('single')
    .description('Send a single transaction')
    .option('-a, --account <name>', 'Account to use (alice, bob, charlie, etc.)', 'alice')
    .option('-r, --remark <text>', 'Remark text', 'Single test transaction')
    .option('-p, --priority <level>', 'Priority level (low, normal, high, critical, max)', 'normal')
    .action(async (options) => {
        const tester = new BlockchainTester(program.opts().endpoint);
        try {
            await tester.connect();
            await tester.singleTransaction(options.account, options.remark, options.priority);
            tester.printStats();
        } finally {
            await tester.disconnect();
        }
    });

program
    .command('serial')
    .description('Send multiple transactions in series')
    .option('-c, --count <number>', 'Number of transactions', '10')
    .option('-a, --account <name>', 'Account to use', 'alice')
    .option('-d, --delay <ms>', 'Delay between transactions (ms)', '100')
    .option('-p, --priority <level>', 'Priority level', 'normal')
    .action(async (options) => {
        const tester = new BlockchainTester(program.opts().endpoint);
        try {
            await tester.connect();
            await tester.serialTransactions(
                parseInt(options.count),
                options.account,
                parseInt(options.delay),
                options.priority
            );
            tester.printStats();
        } finally {
            await tester.disconnect();
        }
    });

program
    .command('parallel')
    .description('Send multiple transactions in parallel')
    .option('-c, --count <number>', 'Number of transactions per account', '5')
    .option('-a, --accounts <names>', 'Comma-separated account names', 'alice,bob,charlie,dave,eve')
    .option('-p, --priority <level>', 'Priority level', 'normal')
    .action(async (options) => {
        const tester = new BlockchainTester(program.opts().endpoint);
        try {
            await tester.connect();
            const accounts = options.accounts.split(',').map(a => a.trim());
            await tester.parallelTransactions(parseInt(options.count), accounts, options.priority);
            tester.printStats();
        } finally {
            await tester.disconnect();
        }
    });

program
    .command('priority')
    .description('Test mixed priority transactions')
    .option('-c, --count <number>', 'Total number of transactions', '10')
    .action(async (options) => {
        const tester = new BlockchainTester(program.opts().endpoint);
        try {
            await tester.connect();
            await tester.priorityTest(parseInt(options.count));
            tester.printStats();
        } finally {
            await tester.disconnect();
        }
    });

program
    .command('stress')
    .description('Stress test with continuous transactions')
    .option('-d, --duration <ms>', 'Test duration in milliseconds', '30000')
    .option('-i, --interval <ms>', 'Interval between transactions (ms)', '100')
    .action(async (options) => {
        const tester = new BlockchainTester(program.opts().endpoint);
        try {
            await tester.connect();
            await tester.stressTest(parseInt(options.duration), parseInt(options.interval));
            tester.printStats();
        } finally {
            await tester.disconnect();
        }
    });

program
    .command('burst')
    .description('Send transactions in bursts')
    .option('-s, --size <number>', 'Transactions per burst', '20')
    .option('-c, --count <number>', 'Number of bursts', '3')
    .option('-i, --interval <ms>', 'Interval between bursts (ms)', '5000')
    .action(async (options) => {
        const tester = new BlockchainTester(program.opts().endpoint);
        try {
            await tester.connect();
            await tester.burstTest(
                parseInt(options.size),
                parseInt(options.count),
                parseInt(options.interval)
            );
            tester.printStats();
        } finally {
            await tester.disconnect();
        }
    });

// Error handling
process.on('unhandledRejection', (error) => {
    console.error('❌ Unhandled promise rejection:', error);
    process.exit(1);
});

process.on('SIGINT', () => {
    console.log('\n🛑 Test interrupted by user');
    process.exit(0);
});

// Parse CLI arguments
program.parse();