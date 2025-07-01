#!/usr/bin/env node

const { ApiPromise, WsProvider, Keyring } = require('@polkadot/api');
const { program } = require('commander');
const fs = require('fs').promises;
const path = require('path');

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

class ThroughputAnalyzer {
    constructor(endpoint = 'ws://127.0.0.1:9944', outputDir = './throughput_results') {
        this.endpoint = endpoint;
        this.outputDir = outputDir;
        this.api = null;
        this.keyring = new Keyring({ type: 'sr25519' });
        this.accounts = {};
        this.testResults = {
            timestamp: new Date().toISOString(),
            endpoint: endpoint,
            tests: [],
            summary: {}
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
        
        // Create output directory
        await this.ensureOutputDir();
    }

    async disconnect() {
        if (this.api) {
            await this.api.disconnect();
            console.log('🔌 Disconnected from blockchain');
        }
    }

    async ensureOutputDir() {
        try {
            await fs.mkdir(this.outputDir, { recursive: true });
        } catch (error) {
            console.warn(`Warning: Could not create output directory: ${error.message}`);
        }
    }

    // Baseline TPS test - single account, varying transaction rates
    async baselineTpsTest(maxTps = 50, testDuration = 30000, stepSize = 5) {
        console.log(`\n📈 Baseline TPS Test`);
        console.log(`Max TPS target: ${maxTps}, Duration: ${testDuration}ms, Step size: ${stepSize}`);
        
        const results = [];
        const signer = this.accounts.alice;
        
        for (let targetTps = stepSize; targetTps <= maxTps; targetTps += stepSize) {
            const interval = Math.floor(1000 / targetTps);
            console.log(`\n🎯 Testing ${targetTps} TPS (${interval}ms interval)...`);
            
            const testResult = await this.runTpsTest(signer, targetTps, interval, testDuration, `baseline-${targetTps}tps`);
            results.push(testResult);
            
            // Brief pause between tests
            await this.sleep(2000);
        }
        
        this.testResults.tests.push({
            testType: 'baseline_tps',
            results,
            summary: this.analyzeTpsResults(results)
        });
        
        return results;
    }

    // Concurrency test - multiple accounts, fixed TPS per account
    async concurrencyTest(accountCount = 6, tpsPerAccount = 10, testDuration = 30000) {
        console.log(`\n⚡ Concurrency Test`);
        console.log(`Accounts: ${accountCount}, TPS per account: ${tpsPerAccount}, Duration: ${testDuration}ms`);
        
        const accountNames = Object.keys(this.accounts).slice(0, accountCount);
        const interval = Math.floor(1000 / tpsPerAccount);
        const promises = [];
        const results = [];
        
        console.log(`📤 Starting ${accountCount} concurrent streams...`);
        
        // Start concurrent TPS tests for each account
        for (const accountName of accountNames) {
            const signer = this.accounts[accountName];
            const promise = this.runTpsTest(
                signer, 
                tpsPerAccount, 
                interval, 
                testDuration, 
                `concurrency-${accountName}`
            );
            promises.push(promise);
        }
        
        // Wait for all concurrent tests to complete
        const concurrentResults = await Promise.allSettled(promises);
        
        concurrentResults.forEach((result, index) => {
            if (result.status === 'fulfilled') {
                results.push(result.value);
            } else {
                console.error(`❌ Account ${accountNames[index]} test failed:`, result.reason);
            }
        });
        
        const totalTps = results.reduce((sum, r) => sum + r.actualTps, 0);
        console.log(`\n📊 Concurrency Test Complete - Total TPS: ${totalTps.toFixed(2)}`);
        
        this.testResults.tests.push({
            testType: 'concurrency',
            accountCount,
            tpsPerAccount,
            results,
            totalTps,
            summary: this.analyzeConcurrencyResults(results)
        });
        
        return results;
    }

    // Maximum sustainable TPS test
    async maxTpsTest(initialTps = 50, maxAttempts = 10, sustainDuration = 15000, successThreshold = 0.95) {
        console.log(`\n🚀 Maximum Sustainable TPS Test`);
        console.log(`Initial TPS: ${initialTps}, Max attempts: ${maxAttempts}, Sustain duration: ${sustainDuration}ms`);
        
        let currentTps = initialTps;
        let bestSustainableTps = 0;
        let attempts = 0;
        const results = [];
        
        while (attempts < maxAttempts) {
            attempts++;
            console.log(`\n🎯 Attempt ${attempts}: Testing ${currentTps} TPS...`);
            
            const signer = this.accounts.alice;
            const interval = Math.floor(1000 / currentTps);
            const testResult = await this.runTpsTest(signer, currentTps, interval, sustainDuration, `max-tps-${currentTps}`);
            
            results.push(testResult);
            
            const successRate = testResult.successRate;
            console.log(`   Success rate: ${(successRate * 100).toFixed(1)}%`);
            
            if (successRate >= successThreshold) {
                bestSustainableTps = currentTps;
                currentTps = Math.floor(currentTps * 1.3); // Increase by 30%
                console.log(`   ✅ Sustainable at ${bestSustainableTps} TPS, trying ${currentTps} TPS next`);
            } else {
                console.log(`   ❌ Not sustainable at ${currentTps} TPS`);
                if (bestSustainableTps === 0) {
                    currentTps = Math.floor(currentTps * 0.7); // Decrease by 30%
                } else {
                    break; // Found maximum
                }
            }
            
            await this.sleep(2000);
        }
        
        console.log(`\n🏆 Maximum sustainable TPS: ${bestSustainableTps}`);
        
        this.testResults.tests.push({
            testType: 'max_tps',
            maxSustainableTps: bestSustainableTps,
            results,
            summary: {
                maxSustainableTps: bestSustainableTps,
                totalAttempts: attempts,
                successThreshold
            }
        });
        
        return bestSustainableTps;
    }

    // Load ramp test - gradually increase load
    async loadRampTest(startTps = 5, endTps = 100, rampDuration = 60000, stepDuration = 5000) {
        console.log(`\n📊 Load Ramp Test`);
        console.log(`Start: ${startTps} TPS, End: ${endTps} TPS, Duration: ${rampDuration}ms`);
        
        const steps = Math.floor(rampDuration / stepDuration);
        const tpsIncrement = (endTps - startTps) / steps;
        const results = [];
        const signer = this.accounts.alice;
        
        for (let step = 0; step < steps; step++) {
            const currentTps = Math.floor(startTps + (tpsIncrement * step));
            const interval = Math.floor(1000 / currentTps);
            
            console.log(`\n📈 Step ${step + 1}/${steps}: ${currentTps} TPS...`);
            
            const testResult = await this.runTpsTest(signer, currentTps, interval, stepDuration, `ramp-step-${step + 1}`);
            results.push(testResult);
        }
        
        this.testResults.tests.push({
            testType: 'load_ramp',
            startTps,
            endTps,
            steps,
            results,
            summary: this.analyzeRampResults(results)
        });
        
        return results;
    }

    // Priority impact test - measure TPS with mixed priorities
    async priorityImpactTest(baseTps = 20, testDuration = 30000) {
        console.log(`\n🏆 Priority Impact Test`);
        console.log(`Base TPS: ${baseTps}, Duration: ${testDuration}ms`);
        
        const priorities = ['low', 'normal', 'high', 'critical'];
        const results = [];
        
        for (const priority of priorities) {
            console.log(`\n🎯 Testing ${priority} priority at ${baseTps} TPS...`);
            
            const signer = this.accounts.alice;
            const interval = Math.floor(1000 / baseTps);
            const testResult = await this.runTpsTest(
                signer, 
                baseTps, 
                interval, 
                testDuration, 
                `priority-${priority}`,
                priority
            );
            
            results.push({ priority, ...testResult });
            await this.sleep(2000);
        }
        
        this.testResults.tests.push({
            testType: 'priority_impact',
            baseTps,
            results,
            summary: this.analyzePriorityResults(results)
        });
        
        return results;
    }

    // Core TPS test runner
    async runTpsTest(signer, targetTps, interval, duration, testId, priority = 'normal') {
        const startTime = Date.now();
        const endTime = startTime + duration;
        let txCounter = 0;
        let successfulTxs = 0;
        let failedTxs = 0;
        const promises = [];
        const timings = [];
        
        // Get initial nonce
        const accountInfo = await this.api.query.system.account(signer.address);
        let currentNonce = accountInfo.nonce.toNumber();
        
        console.log(`   ⏱️  Starting ${testId} test...`);
        
        while (Date.now() < endTime) {
            const txStartTime = Date.now();
            txCounter++;
            
            const remark = `TPS test ${testId} tx #${txCounter}`;
            const priorityValue = PRIORITY_LEVELS[priority] || PRIORITY_LEVELS.normal;
            
            const promise = this.sendTransactionWithNonce(signer, remark, priorityValue, currentNonce)
                .then(hash => {
                    const txEndTime = Date.now();
                    timings.push(txEndTime - txStartTime);
                    successfulTxs++;
                    return { success: true, hash, timing: txEndTime - txStartTime };
                })
                .catch(error => {
                    const txEndTime = Date.now();
                    timings.push(txEndTime - txStartTime);
                    failedTxs++;
                    return { success: false, error: error.message, timing: txEndTime - txStartTime };
                });
            
            promises.push(promise);
            currentNonce++;
            
            // Wait for next transaction
            await this.sleep(interval);
        }
        
        console.log(`   ⏳ Waiting for ${promises.length} transactions to complete...`);
        await Promise.allSettled(promises);
        
        const actualDuration = Date.now() - startTime;
        const actualTps = (txCounter / actualDuration) * 1000;
        const successRate = successfulTxs / txCounter;
        
        const avgTiming = timings.reduce((sum, t) => sum + t, 0) / timings.length;
        const minTiming = Math.min(...timings);
        const maxTiming = Math.max(...timings);
        
        const result = {
            testId,
            targetTps,
            actualTps: parseFloat(actualTps.toFixed(2)),
            totalTxs: txCounter,
            successfulTxs,
            failedTxs,
            successRate: parseFloat(successRate.toFixed(4)),
            duration: actualDuration,
            avgTransactionTime: parseFloat(avgTiming.toFixed(2)),
            minTransactionTime: minTiming,
            maxTransactionTime: maxTiming,
            priority: priority,
            timestamp: new Date().toISOString()
        };
        
        console.log(`   ✅ ${testId}: ${actualTps.toFixed(2)} TPS (${(successRate * 100).toFixed(1)}% success)`);
        
        return result;
    }

    // Transaction sender with nonce management
    async sendTransactionWithNonce(signer, remark, priority = PRIORITY_LEVELS.normal, nonce) {
        return new Promise((resolve, reject) => {
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

    // Analysis methods
    analyzeTpsResults(results) {
        const successfulTests = results.filter(r => r.successRate >= 0.95);
        const maxTps = Math.max(...results.map(r => r.actualTps));
        const maxSustainableTps = successfulTests.length > 0 ? 
            Math.max(...successfulTests.map(r => r.actualTps)) : 0;
        
        return {
            maxTps,
            maxSustainableTps,
            totalTests: results.length,
            successfulTests: successfulTests.length,
            avgSuccessRate: results.reduce((sum, r) => sum + r.successRate, 0) / results.length
        };
    }

    analyzeConcurrencyResults(results) {
        const totalTps = results.reduce((sum, r) => sum + r.actualTps, 0);
        const avgSuccessRate = results.reduce((sum, r) => sum + r.successRate, 0) / results.length;
        const minTps = Math.min(...results.map(r => r.actualTps));
        const maxTps = Math.max(...results.map(r => r.actualTps));
        
        return {
            totalTps,
            avgSuccessRate,
            minTps,
            maxTps,
            accountCount: results.length
        };
    }

    analyzeRampResults(results) {
        const breakingPoint = results.findIndex(r => r.successRate < 0.95);
        const sustainableTps = breakingPoint > 0 ? results[breakingPoint - 1].actualTps : 0;
        
        return {
            sustainableTps,
            breakingPointStep: breakingPoint,
            maxAttemptedTps: Math.max(...results.map(r => r.actualTps)),
            avgSuccessRate: results.reduce((sum, r) => sum + r.successRate, 0) / results.length
        };
    }

    analyzePriorityResults(results) {
        const priorityComparison = {};
        results.forEach(r => {
            priorityComparison[r.priority] = {
                actualTps: r.actualTps,
                successRate: r.successRate,
                avgTransactionTime: r.avgTransactionTime
            };
        });
        
        return { priorityComparison };
    }

    // Comprehensive test suite
    async runFullAnalysis(options = {}) {
        const {
            baselineMaxTps = 50,
            concurrencyAccounts = 6,
            concurrencyTps = 10,
            testDuration = 30000
        } = options;
        
        console.log(`\n🔬 Starting Full Throughput Analysis`);
        console.log(`================================================`);
        
        try {
            // 1. Baseline TPS test
            await this.baselineTpsTest(baselineMaxTps, testDuration);
            
            // 2. Concurrency test
            await this.concurrencyTest(concurrencyAccounts, concurrencyTps, testDuration);
            
            // 3. Maximum TPS test
            await this.maxTpsTest();
            
            // 4. Load ramp test
            await this.loadRampTest(5, 50, 60000, 5000);
            
            // 5. Priority impact test
            await this.priorityImpactTest(20, testDuration);
            
            // Generate comprehensive summary
            this.generateComprehensiveSummary();
            
            // Save results
            await this.saveResults();
            
        } catch (error) {
            console.error(`❌ Analysis failed:`, error);
            throw error;
        }
    }

    generateComprehensiveSummary() {
        const summary = {
            timestamp: new Date().toISOString(),
            endpoint: this.endpoint
        };
        
        // Extract key metrics from each test
        this.testResults.tests.forEach(test => {
            switch (test.testType) {
                case 'baseline_tps':
                    summary.baselineMaxSustainableTps = test.summary.maxSustainableTps;
                    break;
                case 'concurrency':
                    summary.maxConcurrentTps = test.totalTps;
                    summary.concurrencyScaling = test.totalTps / (test.accountCount * test.tpsPerAccount);
                    break;
                case 'max_tps':
                    summary.absoluteMaxTps = test.maxSustainableTps;
                    break;
                case 'load_ramp':
                    summary.rampTestSustainableTps = test.summary.sustainableTps;
                    break;
                case 'priority_impact':
                    summary.priorityImpact = test.summary.priorityComparison;
                    break;
            }
        });
        
        this.testResults.summary = summary;
        
        console.log(`\n📊 COMPREHENSIVE ANALYSIS SUMMARY`);
        console.log(`================================================`);
        console.log(`🎯 Baseline Max Sustainable TPS: ${summary.baselineMaxSustainableTps || 'N/A'}`);
        console.log(`⚡ Max Concurrent TPS: ${summary.maxConcurrentTps || 'N/A'}`);
        console.log(`🚀 Absolute Max TPS: ${summary.absoluteMaxTps || 'N/A'}`);
        console.log(`📈 Ramp Test Sustainable TPS: ${summary.rampTestSustainableTps || 'N/A'}`);
        console.log(`⚖️  Concurrency Scaling: ${(summary.concurrencyScaling * 100 || 0).toFixed(1)}%`);
        console.log(`================================================`);
    }

    async saveResults() {
        const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
        const filename = `throughput_analysis_${timestamp}.json`;
        const filepath = path.join(this.outputDir, filename);
        
        try {
            await fs.writeFile(filepath, JSON.stringify(this.testResults, null, 2));
            console.log(`💾 Results saved to: ${filepath}`);
            
            // Also save a human-readable summary
            const summaryFilename = `throughput_summary_${timestamp}.txt`;
            const summaryFilepath = path.join(this.outputDir, summaryFilename);
            const summaryText = this.generateTextSummary();
            await fs.writeFile(summaryFilepath, summaryText);
            console.log(`📄 Summary saved to: ${summaryFilepath}`);
            
        } catch (error) {
            console.error(`❌ Failed to save results:`, error);
        }
    }

    generateTextSummary() {
        const summary = this.testResults.summary;
        const timestamp = new Date().toISOString();
        
        return `
MetaMUI Blockchain Throughput Analysis Report
============================================
Generated: ${timestamp}
Endpoint: ${this.endpoint}

EXECUTIVE SUMMARY
================
• Baseline Max Sustainable TPS: ${summary.baselineMaxSustainableTps || 'N/A'}
• Maximum Concurrent TPS: ${summary.maxConcurrentTps || 'N/A'}
• Absolute Maximum TPS: ${summary.absoluteMaxTps || 'N/A'}
• Ramp Test Sustainable TPS: ${summary.rampTestSustainableTps || 'N/A'}
• Concurrency Scaling Efficiency: ${(summary.concurrencyScaling * 100 || 0).toFixed(1)}%

DETAILED RESULTS
===============
${this.testResults.tests.map(test => {
    switch (test.testType) {
        case 'baseline_tps':
            return `
Baseline TPS Test:
- Tests conducted: ${test.summary.totalTests}
- Maximum TPS achieved: ${test.summary.maxTps}
- Maximum sustainable TPS: ${test.summary.maxSustainableTps}
- Average success rate: ${(test.summary.avgSuccessRate * 100).toFixed(1)}%`;
        case 'concurrency':
            return `
Concurrency Test:
- Accounts tested: ${test.accountCount}
- TPS per account: ${test.tpsPerAccount}
- Total TPS achieved: ${test.totalTps.toFixed(2)}
- Scaling efficiency: ${(test.totalTps / (test.accountCount * test.tpsPerAccount) * 100).toFixed(1)}%`;
        case 'max_tps':
            return `
Maximum TPS Test:
- Maximum sustainable TPS: ${test.maxSustainableTps}
- Total attempts: ${test.summary.totalAttempts}`;
        case 'load_ramp':
            return `
Load Ramp Test:
- Sustainable TPS: ${test.summary.sustainableTps}
- Maximum attempted TPS: ${test.summary.maxAttemptedTps}`;
        case 'priority_impact':
            return `
Priority Impact Test:
${Object.entries(test.summary.priorityComparison).map(([priority, data]) => 
    `- ${priority.toUpperCase()}: ${data.actualTps} TPS (${(data.successRate * 100).toFixed(1)}% success)`
).join('\n')}`;
        default:
            return '';
    }
}).join('\n')}

RECOMMENDATIONS
==============
Based on the analysis:
1. For sustained operation, limit TPS to ${Math.floor((summary.baselineMaxSustainableTps || 0) * 0.8)}
2. For burst loads, maximum TPS is ${summary.absoluteMaxTps || 'unknown'}
3. Concurrency scaling is ${summary.concurrencyScaling > 0.8 ? 'excellent' : summary.concurrencyScaling > 0.6 ? 'good' : 'needs improvement'}
4. Priority settings ${summary.priorityImpact ? 'do' : 'do not'} significantly impact throughput

This report was generated by MetaMUI Throughput Analyzer v1.0
`;
    }

    sleep(ms) {
        return new Promise(resolve => setTimeout(resolve, ms));
    }
}

// CLI Configuration
program
    .name('throughput-analyzer')
    .description('Comprehensive blockchain throughput and TPS analysis tool')
    .version('1.0.0')
    .option('-e, --endpoint <url>', 'Blockchain endpoint', 'ws://127.0.0.1:9944')
    .option('-o, --output <dir>', 'Output directory for results', './throughput_results');

program
    .command('baseline')
    .description('Run baseline TPS test')
    .option('-m, --max-tps <number>', 'Maximum TPS to test', '50')
    .option('-d, --duration <ms>', 'Test duration per TPS level (ms)', '30000')
    .option('-s, --step <number>', 'TPS step size', '5')
    .action(async (options) => {
        const analyzer = new ThroughputAnalyzer(program.opts().endpoint, program.opts().output);
        try {
            await analyzer.connect();
            await analyzer.baselineTpsTest(
                parseInt(options.maxTps),
                parseInt(options.duration),
                parseInt(options.step)
            );
            await analyzer.saveResults();
        } finally {
            await analyzer.disconnect();
        }
    });

program
    .command('concurrency')
    .description('Run concurrency test')
    .option('-a, --accounts <number>', 'Number of accounts', '6')
    .option('-t, --tps <number>', 'TPS per account', '10')
    .option('-d, --duration <ms>', 'Test duration (ms)', '30000')
    .action(async (options) => {
        const analyzer = new ThroughputAnalyzer(program.opts().endpoint, program.opts().output);
        try {
            await analyzer.connect();
            await analyzer.concurrencyTest(
                parseInt(options.accounts),
                parseInt(options.tps),
                parseInt(options.duration)
            );
            await analyzer.saveResults();
        } finally {
            await analyzer.disconnect();
        }
    });

program
    .command('max-tps')
    .description('Find maximum sustainable TPS')
    .option('-i, --initial <number>', 'Initial TPS', '50')
    .option('-m, --max-attempts <number>', 'Maximum attempts', '10')
    .option('-d, --duration <ms>', 'Test duration (ms)', '15000')
    .option('-t, --threshold <number>', 'Success threshold (0-1)', '0.95')
    .action(async (options) => {
        const analyzer = new ThroughputAnalyzer(program.opts().endpoint, program.opts().output);
        try {
            await analyzer.connect();
            await analyzer.maxTpsTest(
                parseInt(options.initial),
                parseInt(options.maxAttempts),
                parseInt(options.duration),
                parseFloat(options.threshold)
            );
            await analyzer.saveResults();
        } finally {
            await analyzer.disconnect();
        }
    });

program
    .command('ramp')
    .description('Run load ramp test')
    .option('-s, --start-tps <number>', 'Starting TPS', '5')
    .option('-e, --end-tps <number>', 'Ending TPS', '100')
    .option('-d, --duration <ms>', 'Total ramp duration (ms)', '60000')
    .option('-t, --step-duration <ms>', 'Duration per step (ms)', '5000')
    .action(async (options) => {
        const analyzer = new ThroughputAnalyzer(program.opts().endpoint, program.opts().output);
        try {
            await analyzer.connect();
            await analyzer.loadRampTest(
                parseInt(options.startTps),
                parseInt(options.endTps),
                parseInt(options.duration),
                parseInt(options.stepDuration)
            );
            await analyzer.saveResults();
        } finally {
            await analyzer.disconnect();
        }
    });

program
    .command('priority')
    .description('Test priority impact on TPS')
    .option('-t, --tps <number>', 'Base TPS', '20')
    .option('-d, --duration <ms>', 'Test duration (ms)', '30000')
    .action(async (options) => {
        const analyzer = new ThroughputAnalyzer(program.opts().endpoint, program.opts().output);
        try {
            await analyzer.connect();
            await analyzer.priorityImpactTest(
                parseInt(options.tps),
                parseInt(options.duration)
            );
            await analyzer.saveResults();
        } finally {
            await analyzer.disconnect();
        }
    });

program
    .command('full')
    .description('Run comprehensive throughput analysis')
    .option('-m, --max-tps <number>', 'Maximum TPS for baseline test', '50')
    .option('-a, --accounts <number>', 'Number of accounts for concurrency', '6')
    .option('-t, --concurrency-tps <number>', 'TPS per account for concurrency', '10')
    .option('-d, --duration <ms>', 'Test duration (ms)', '30000')
    .action(async (options) => {
        const analyzer = new ThroughputAnalyzer(program.opts().endpoint, program.opts().output);
        try {
            await analyzer.connect();
            await analyzer.runFullAnalysis({
                baselineMaxTps: parseInt(options.maxTps),
                concurrencyAccounts: parseInt(options.accounts),
                concurrencyTps: parseInt(options.concurrencyTps),
                testDuration: parseInt(options.duration)
            });
        } finally {
            await analyzer.disconnect();
        }
    });

// Error handling
process.on('unhandledRejection', (error) => {
    console.error('❌ Unhandled promise rejection:', error);
    process.exit(1);
});

process.on('SIGINT', () => {
    console.log('\n🛑 Analysis interrupted by user');
    process.exit(0);
});

// Parse CLI arguments
program.parse();