import * as path from 'path';
import * as fs from 'fs';
import Mocha from 'mocha';

function collectTestFiles(dir: string): string[] {
    const results: string[] = [];
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
        const full = path.join(dir, entry.name);
        if (entry.isDirectory()) {
            results.push(...collectTestFiles(full));
        } else if (entry.name.endsWith('.test.js')) {
            results.push(full);
        }
    }
    return results;
}

export async function run(): Promise<void> {
    const mocha = new Mocha({
        ui: 'tdd',
        color: true,
        timeout: 15000
    });

    const testsRoot = path.resolve(__dirname);

    const files = collectTestFiles(testsRoot);
    for (const file of files) {
        mocha.addFile(file);
    }

    return new Promise((resolve, reject) => {
        mocha.run((failures: number) => {
            if (failures > 0) {
                reject(new Error(`${failures} tests failed.`));
            } else {
                resolve();
            }
        });
    });
}
