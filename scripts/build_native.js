const { spawnSync } = require('child_process');

const commands = process.platform === 'win32' ? ['py', 'python', 'python3'] : ['python3', 'python'];

for (const command of commands) {
    const result = spawnSync(command, ['scripts/build_current.py', ...process.argv.slice(2)], {
        stdio: 'inherit'
    });

    if (result.error?.code === 'ENOENT') {
        continue;
    }
    if (result.error) {
        throw result.error;
    }
    process.exit(result.status ?? 1);
}

console.error(`Python not found (tried: ${commands.join(', ')})`);
process.exit(1);
