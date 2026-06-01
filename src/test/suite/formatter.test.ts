import * as assert from 'assert';
import * as path from 'path';
import * as os from 'os';
import * as fs from 'fs';
import { RustFormatter, FormatterConfig } from '../../formatter';

const TEST_CONFIG: FormatterConfig = {
    rustfmtPath: 'rustfmt',
    extraArgs: []
};

suite('RustFormatter', () => {
    let formatter: RustFormatter;
    let workspaceRoot: string;

    setup(() => {
        formatter = new RustFormatter(TEST_CONFIG);
        workspaceRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'rust-fmt-test-'));
    });

    teardown(() => {
        fs.rmSync(workspaceRoot, { recursive: true, force: true });
    });

    suite('resolveContext', () => {

        test('resolves crateRoot from Cargo.toml', async () => {
            fs.writeFileSync(path.join(workspaceRoot, 'Cargo.toml'), '[package]\nname = "test"\nedition = "2021"\n');
            const testFile = path.join(workspaceRoot, 'src', 'lib.rs');
            fs.mkdirSync(path.dirname(testFile), { recursive: true });
            fs.writeFileSync(testFile, 'fn main() {}');

            const ctx = await formatter.resolveContext(testFile, workspaceRoot);

            assert.strictEqual(ctx.crateRoot, workspaceRoot);
            assert.strictEqual(ctx.edition, '2021');
            assert.strictEqual(ctx.cwd, workspaceRoot);
        });

        test('resolves edition from Cargo.toml', async () => {
            fs.writeFileSync(path.join(workspaceRoot, 'Cargo.toml'), '[package]\nname = "test"\nedition = "2018"\n');
            const testFile = path.join(workspaceRoot, 'src', 'main.rs');
            fs.mkdirSync(path.dirname(testFile), { recursive: true });
            fs.writeFileSync(testFile, 'fn main() {}');

            const ctx = await formatter.resolveContext(testFile, workspaceRoot);

            assert.strictEqual(ctx.edition, '2018');
        });

        test('resolves rustfmt.toml config path', async () => {
            fs.writeFileSync(path.join(workspaceRoot, 'Cargo.toml'), '[package]\nname = "test"\n');
            fs.writeFileSync(path.join(workspaceRoot, 'rustfmt.toml'), 'max_width = 100\n');
            const testFile = path.join(workspaceRoot, 'src', 'main.rs');
            fs.mkdirSync(path.dirname(testFile), { recursive: true });
            fs.writeFileSync(testFile, 'fn main() {}');

            const ctx = await formatter.resolveContext(testFile, workspaceRoot);

            assert.ok(ctx.configPath, 'configPath should be set');
            assert.ok(ctx.configPath!.endsWith('rustfmt.toml'), `Expected configPath to end with rustfmt.toml, got: ${ctx.configPath}`);
        });

        test('resolves .rustfmt.toml config path', async () => {
            fs.writeFileSync(path.join(workspaceRoot, 'Cargo.toml'), '[package]\nname = "test"\n');
            fs.writeFileSync(path.join(workspaceRoot, '.rustfmt.toml'), 'max_width = 80\n');
            const testFile = path.join(workspaceRoot, 'src', 'main.rs');
            fs.mkdirSync(path.dirname(testFile), { recursive: true });
            fs.writeFileSync(testFile, 'fn main() {}');

            const ctx = await formatter.resolveContext(testFile, workspaceRoot);

            assert.ok(ctx.configPath, 'configPath should be set');
            assert.ok(ctx.configPath!.endsWith('.rustfmt.toml'), `Expected configPath to end with .rustfmt.toml, got: ${ctx.configPath}`);
        });

        test('resolves toolchain from rust-toolchain.toml', async () => {
            fs.writeFileSync(path.join(workspaceRoot, 'Cargo.toml'), '[package]\nname = "test"\n');
            fs.writeFileSync(path.join(workspaceRoot, 'rust-toolchain.toml'), '[toolchain]\nchannel = "nightly"\n');
            const testFile = path.join(workspaceRoot, 'src', 'main.rs');
            fs.mkdirSync(path.dirname(testFile), { recursive: true });
            fs.writeFileSync(testFile, 'fn main() {}');

            const ctx = await formatter.resolveContext(testFile, workspaceRoot);

            assert.strictEqual(ctx.toolchain, 'nightly');
        });

        test('resolves toolchain from plain rust-toolchain file', async () => {
            fs.writeFileSync(path.join(workspaceRoot, 'Cargo.toml'), '[package]\nname = "test"\n');
            fs.writeFileSync(path.join(workspaceRoot, 'rust-toolchain'), 'stable\n');
            const testFile = path.join(workspaceRoot, 'src', 'main.rs');
            fs.mkdirSync(path.dirname(testFile), { recursive: true });
            fs.writeFileSync(testFile, 'fn main() {}');

            const ctx = await formatter.resolveContext(testFile, workspaceRoot);

            assert.strictEqual(ctx.toolchain, 'stable');
        });

        test('returns undefined crateRoot when no Cargo.toml found', async () => {
            const testFile = path.join(workspaceRoot, 'standalone.rs');
            fs.writeFileSync(testFile, 'fn main() {}');

            const ctx = await formatter.resolveContext(testFile, workspaceRoot);

            assert.strictEqual(ctx.crateRoot, undefined);
            assert.strictEqual(ctx.edition, undefined);
            assert.strictEqual(ctx.cwd, workspaceRoot);
        });

        test('searches upward for Cargo.toml in nested dirs', async () => {
            fs.writeFileSync(path.join(workspaceRoot, 'Cargo.toml'), '[package]\nname = "test"\nedition = "2021"\n');
            const deepDir = path.join(workspaceRoot, 'src', 'deep', 'nested');
            fs.mkdirSync(deepDir, { recursive: true });
            const testFile = path.join(deepDir, 'mod.rs');
            fs.writeFileSync(testFile, 'pub fn foo() {}');

            const ctx = await formatter.resolveContext(testFile, workspaceRoot);

            assert.strictEqual(ctx.crateRoot, workspaceRoot);
            assert.strictEqual(ctx.edition, '2021');
        });

        test('stops searching at workspaceFolder boundary', async () => {
            const innerDir = path.join(workspaceRoot, 'project');
            fs.mkdirSync(innerDir, { recursive: true });
            fs.writeFileSync(path.join(workspaceRoot, 'Cargo.toml'), '[package]\nname = "outer"\nedition = "2021"\n');
            const testFile = path.join(innerDir, 'main.rs');
            fs.writeFileSync(testFile, 'fn main() {}');

            const ctx = await formatter.resolveContext(testFile, innerDir);

            assert.strictEqual(ctx.crateRoot, undefined, 'should not find Cargo.toml above workspaceFolder');
        });
    });

    suite('config', () => {

        test('updateConfig does not throw', () => {
            const newConfig: FormatterConfig = { rustfmtPath: '/custom/rustfmt', extraArgs: ['--verbose'] };
            formatter.updateConfig(newConfig);
            assert.ok(true);
        });
    });
});
