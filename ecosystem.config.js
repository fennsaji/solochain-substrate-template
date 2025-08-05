module.exports = {
  apps: [
    {
      name: 'node-1',
      script: '/Users/fennsaji/Documents/Projects/Fenn/solochain-substrate-template/target/release/solochain-template-node',
      args: [
        '--validator',
        '--base-path', '/tmp/solochain-nodes/node1',
        '--chain', '/Users/fennsaji/Documents/Projects/Fenn/solochain-substrate-template/chainspecs/local/specRaw.json',
        '--port', '30333',
        '--rpc-port', '9944',
        '--node-key', 'c12b6d18942f5ee8528c8e2baf4e147b5c5c18710926ea492d09cbd9f6c9f82a',
        '--rpc-external',
        '--rpc-cors', 'all',
        '--rpc-methods=Unsafe',
        '--name', 'node-1',
        '--pruning', 'archive'
      ]
    },
    {
      name: 'node-2',
      script: '/Users/fennsaji/Documents/Projects/Fenn/solochain-substrate-template/target/release/solochain-template-node',
      args: [
        '--validator',
        '--base-path', '/tmp/solochain-nodes/node2',
        '--chain', '/Users/fennsaji/Documents/Projects/Fenn/solochain-substrate-template/chainspecs/local/specRaw.json',
        '--bootnodes', '/ip4/127.0.0.1/tcp/30333/p2p/12D3KooWBmAwcd4PJNJvfV89HwE48nwkRmAgo8Vy3uQEyNNHBox2',
        '--port', '30334',
        '--rpc-port', '9945',
        '--rpc-external',
        '--rpc-cors', 'all',
        '--rpc-methods=Unsafe',
        '--node-key', '0000000000000000000000000000000000000000000000000000000000000002',
        '--name', 'node-2',
        '--pruning', 'archive'
      ]
    },
    {
      name: 'node-3',
      script: '/Users/fennsaji/Documents/Projects/Fenn/solochain-substrate-template/target/release/solochain-template-node',
      args: [
        '--validator',
        '--base-path', '/tmp/solochain-nodes/node3',
        '--chain', '/Users/fennsaji/Documents/Projects/Fenn/solochain-substrate-template/chainspecs/local/specRaw.json',
        '--bootnodes', '/ip4/127.0.0.1/tcp/30333/p2p/12D3KooWBmAwcd4PJNJvfV89HwE48nwkRmAgo8Vy3uQEyNNHBox2',
        '--port', '30335',
        '--rpc-port', '9946',
        '--rpc-external',
        '--rpc-cors', 'all',
        '--rpc-methods=Unsafe',
        '--node-key', '0000000000000000000000000000000000000000000000000000000000000003',
        '--name', 'node-3',
        '--pruning', 'archive'
      ]
    }
  ]
};
