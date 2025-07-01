#!/usr/bin/env python3
"""
Check if off-chain worker is running by examining off-chain storage
"""

import requests
import json

def get_current_block():
    """Get current block number"""
    payload = {
        "jsonrpc": "2.0",
        "method": "chain_getHeader", 
        "params": [],
        "id": 1
    }
    response = requests.post("http://localhost:9944/", json=payload)
    if response.status_code == 200:
        result = response.json()
        if "result" in result and result["result"]:
            block_hex = result["result"]["number"]
            return int(block_hex, 16)
    return None

def check_rebase_metadata():
    """Check rebase metadata"""
    payload = {
        "jsonrpc": "2.0",
        "method": "rebase_get_metadata",
        "params": [],
        "id": 1
    }
    response = requests.post("http://localhost:9944/", json=payload)
    if response.status_code == 200:
        result = response.json()
        if "result" in result:
            return result["result"]
    return None

def main():
    print("🔍 Checking Off-chain Worker Status")
    print("=" * 50)
    
    # Check current block
    current_block = get_current_block()
    if current_block:
        print(f"📦 Current Block: #{current_block}")
        
        # Calculate expected rebases
        expected_rebases = current_block // 100
        print(f"🎯 Expected Rebases: {expected_rebases} (every 100 blocks)")
        
        # Check rebase metadata
        metadata = check_rebase_metadata()
        if metadata:
            print(f"📊 Actual Rebases: {metadata.get('rebase_count', 0)}")
            print(f"📍 Last Rebase Block: {metadata.get('last_rebase_block', 0)}")
            print(f"⏭️  Next Scheduled: {metadata.get('next_scheduled_rebase', 'None')}")
            
            if metadata.get('rebase_count', 0) > 0:
                print("✅ OFF-CHAIN WORKER IS WORKING - Rebases detected!")
            else:
                print("❌ OFF-CHAIN WORKER NOT TRIGGERING - No rebases detected")
                print("   Possible issues:")
                print("   - Off-chain worker not enabled")
                print("   - Off-chain worker pallet not running")
                print("   - Logic error in rebase triggering")
        else:
            print("❌ Could not get rebase metadata")
    else:
        print("❌ Could not connect to node")

if __name__ == "__main__":
    main()