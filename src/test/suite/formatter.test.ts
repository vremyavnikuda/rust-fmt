import * as assert from 'assert';
import * as path from 'path';
import * as os from 'os';
import * as fs from 'fs';
import * as vscode from 'vscode';
import { RustFormatter, FormatterConfig, normalizeMacroSpacing, normalizeMacroBodies, getNativeMacroFormatterPath, formatWithNativeMacroFormatter, sha256File } from '../../formatter';
import { changedLineBlocks } from '../../checkFormatting';

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

    suite('safe formatter boundary', () => {

        test('does not rewrite literal or comment bytes', () => {
            const input = 'const S: &str = "a  b"; // keep  two spaces\n';
            assert.strictEqual(normalizeMacroSpacing(input), input);
            assert.strictEqual(normalizeMacroBodies(input), input);
        });

        test('computes the selected native binary hash', async () => {
            const file = path.join(workspaceRoot, 'binary');
            fs.writeFileSync(file, 'abc');
            assert.strictEqual(
                await sha256File(file),
                'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad'
            );
        });
    });

    suite.skip('legacy normalizeMacroSpacing behavior', () => {

        test('preserves whitespace inside string literals', () => {
            const input = 'const S: &str = "a  b";';
            assert.strictEqual(normalizeMacroSpacing(input), input);
        });

        test('collapses extra spaces after !( in macro invocations', () => {
            const input = 'println!(  "hello")';
            const expected = 'println!("hello")';
            assert.strictEqual(normalizeMacroSpacing(input), expected);
        });

        test('collapses extra spaces after ![ in macro invocations', () => {
            const input = 'vec![  1, 2, 3]';
            const expected = 'vec![1, 2, 3]';
            assert.strictEqual(normalizeMacroSpacing(input), expected);
        });

        test('collapses extra spaces after !{ in macro invocations', () => {
            const input = 'my_macro!{  key: value}';
            const expected = 'my_macro!{key: value}';
            assert.strictEqual(normalizeMacroSpacing(input), expected);
        });

        test('collapses multiple spaces between ! and next token', () => {
            const input = 'my_macro!  SomeToken';
            const expected = 'my_macro! SomeToken';
            assert.strictEqual(normalizeMacroSpacing(input), expected);
        });

        test('collapses 2+ spaces before { inside macro body', () => {
            const input = 'define_enum!(MyGeneratedEnum  {';
            const expected = 'define_enum!(MyGeneratedEnum {';
            assert.strictEqual(normalizeMacroSpacing(input), expected);
        });

        test('collapses 2+ spaces before ( inside macro body', () => {
            const input = 'some_macro!(SomeToken  (  inner))';
            const expected = 'some_macro!(SomeToken (inner))';
            assert.strictEqual(normalizeMacroSpacing(input), expected);
        });

        test('collapses 2+ spaces before [ inside macro body', () => {
            const input = 'vec![1,  2,  3]';
            const expected = 'vec![1, 2, 3]';
            assert.strictEqual(normalizeMacroSpacing(input), expected);
        });

        test('handles macro_rules! patterns with extra spacing', () => {
            const input = 'macro_rules!  foo  {  ($x:expr)  =>  {  $x  }  }';
            const result = normalizeMacroSpacing(input);
            assert.ok(!result.includes('  '), 'should have no double spaces');
        });

        test('preserves single spaces', () => {
            const input = 'println!("hello world")';
            assert.strictEqual(normalizeMacroSpacing(input), input);
        });

        test('handles multi-character macro names', () => {
            const input = 'some_long_macro_name!("arg1",  "arg2")';
            const expected = 'some_long_macro_name!("arg1", "arg2")';
            assert.strictEqual(normalizeMacroSpacing(input), expected);
        });

        test('preserves newlines (does not eat line breaks)', () => {
            const input = 'println!(\n    "hello",\n    "world"\n)';
            assert.strictEqual(normalizeMacroSpacing(input), input);
        });

        test('tabs are not collapsed', () => {
            const input = 'some_macro!(\t\thell)';
            assert.strictEqual(normalizeMacroSpacing(input), input);
        });
    });

    suite.skip('legacy normalizeMacroBodies behavior', () => {

        test('normalizes over-indented macro body to expected level', () => {
            const input = [
                'macro_rules! foo { () => {',
                '            let x = 1;',
                '    };',
                '}'
            ].join('\n');
            const output = normalizeMacroBodies(input);
            const lines = output.split('\n');
            assert.strictEqual(lines[1], '    let x = 1;');
        });

        test('preserves correctly indented macro body', () => {
            const input = [
                'macro_rules! foo {',
                '    ($x:expr) => {',
                '        println!("{}", $x)',
                '    };',
                '}'
            ].join('\n');
            assert.strictEqual(normalizeMacroBodies(input), input);
        });

        test('preserves closing line indent', () => {
            const input = [
                'macro_rules! foo { () => {',
                '            let x = 1;',
                '    };',
                '}'
            ].join('\n');
            const output = normalizeMacroBodies(input);
            const lines = output.split('\n');
            assert.strictEqual(lines[2], '    };');
        });

        test('normalizes field_accessor-style macro body with nesting', () => {
            const input = [
                'macro_rules! field_accessor { ( $name:ident, $($f:ident : $t:ty),+ ) => {',
                '            impl $name {',
                '            $(',
                '                pub fn $f(&self) -> &$t {',
                '                        &self.$f',
                '                }',
                '            )+',
                '        }',
                '    };',
                '}'
            ].join('\n');
            const output = normalizeMacroBodies(input);
            const lines = output.split('\n');
            assert.strictEqual(lines[1], '    impl $name {');
            assert.strictEqual(lines[2], '        $(');
            assert.strictEqual(lines[3], '            pub fn $f(&self) -> &$t {');
            assert.strictEqual(lines[4], '                &self.$f');
            assert.strictEqual(lines[5], '            }');
            assert.strictEqual(lines[6], '        )+');
            assert.strictEqual(lines[7], '    }');
        });

        test('does not modify non-macro code', () => {
            const input = [
                'fn main() {',
                '    let x = 1;',
                '    println!("{}", x);',
                '}'
            ].join('\n');
            assert.strictEqual(normalizeMacroBodies(input), input);
        });

        test('handles complex_pattern with multi-arm on same line', () => {
            const input = [
                'macro_rules! complex_pattern { ( @inner $a:ident : $b:expr ) => {',
                '            let $a = $b;',
                '    }; ( $name:ident { $($f:ident : $v:expr),+ } ) => {',
                '            let $name = ( $($v),+ );',
                '    };',
                '}'
            ].join('\n');
            const output = normalizeMacroBodies(input);
            const lines = output.split('\n');
            assert.strictEqual(lines[1], '    let $a = $b;');
            assert.strictEqual(lines[2], '    }; ( $name:ident { $($f:ident : $v:expr),+ } ) => {');
            assert.strictEqual(lines[3], '        let $name = ( $($v),+ );');
        });

        test('handles run_length_encode with double brace', () => {
            const input = [
                'macro_rules! rle { ( $($x:expr),* ) => {{',
                '            let mut v = Vec::new();',
                '        v.push(1);',
                '    }};',
                '}'
            ].join('\n');
            const output = normalizeMacroBodies(input);
            const lines = output.split('\n');
            assert.strictEqual(lines[1], '    let mut v = Vec::new();');
            assert.strictEqual(lines[2], '    v.push(1);');
            assert.strictEqual(lines[3], '    }};');
        });
    });

    suite('getNativeMacroFormatterPath', () => {

        test('returns explicit path when configured', () => {
            const config: FormatterConfig = {
                rustfmtPath: 'rustfmt',
                extraArgs: [],
                nativeMacroFormatter: true,
                nativeMacroFormatterPath: '/custom/path/rust-fmt-mf'
            };
            const result = getNativeMacroFormatterPath(config);
            assert.strictEqual(result, '/custom/path/rust-fmt-mf');
        });

        test('returns null when native is not enabled', () => {
            const config: FormatterConfig = {
                rustfmtPath: 'rustfmt',
                extraArgs: [],
                nativeMacroFormatter: false
            };
            const result = getNativeMacroFormatterPath(config);
            assert.strictEqual(result, null);
        });

        test('returns bundled binary path when native enabled and no explicit path', () => {
            const config: FormatterConfig = {
                rustfmtPath: 'rustfmt',
                extraArgs: [],
                nativeMacroFormatter: true
            };
            const result = getNativeMacroFormatterPath(config);
            assert.ok(result, 'should return a path');
            assert.ok(result!.endsWith('rust-fmt-mf') || result!.endsWith('rust-fmt-mf.exe'), `expected binary path, got: ${result}`);
        });
    });

    suite('formatWithNativeMacroFormatter', () => {

        test('formats via bundled native macro formatter', async () => {
            const config: FormatterConfig = {
                rustfmtPath: 'rustfmt',
                extraArgs: [],
                nativeMacroFormatter: true
            };
            const context = { cwd: undefined };
            const result = await formatWithNativeMacroFormatter('fn main() {}', config, context);
            assert.ok(result.ok, `should return formatted output, got: ${JSON.stringify(result)}`);
            assert.ok(result.ok && result.text.includes('fn main()'), `expected formatted fn main, got: ${JSON.stringify(result)}`);
        });

        test('reports cancellation instead of a bare failure', async () => {
            const config: FormatterConfig = {
                rustfmtPath: 'rustfmt',
                extraArgs: [],
                nativeMacroFormatter: true,
                nativeMacroFormatterPath: '/some/path'
            };
            const source = new vscode.CancellationTokenSource();
            source.cancel();
            const context = { cwd: undefined };
            const result = await formatWithNativeMacroFormatter('fn main() {}', config, context, source.token);
            assert.deepStrictEqual(result, { ok: false, reason: 'canceled' });
        });

        test('reports a missing binary distinctly from cancellation', async () => {
            const config: FormatterConfig = {
                rustfmtPath: 'rustfmt',
                extraArgs: []
            };
            const result = await formatWithNativeMacroFormatter('fn main() {}', config, { cwd: undefined });
            assert.deepStrictEqual(result, { ok: false, reason: 'missing-binary' });
        });

        test('RustFormatter uses native formatting for ordinary Rust files', async () => {
            const nativePath = path.join(workspaceRoot, 'native-formatter');
            fs.writeFileSync(nativePath, '#!/bin/sh\nsed "s/fn main/fn native_main/"\n');
            fs.chmodSync(nativePath, 0o755);
            const nativeFormatter = new RustFormatter({
                rustfmtPath: 'rustfmt',
                extraArgs: [],
                nativeMacroFormatter: true,
                nativeMacroFormatterPath: nativePath
            });

            const result = await nativeFormatter.formatWithContext(
                'fn main() {}\n',
                { cwd: workspaceRoot }
            );

            assert.strictEqual(result, 'fn native_main() {}\n');
        });
    });
});

suite('changedLineBlocks', () => {

    test('reports nothing for identical text', () => {
        assert.deepStrictEqual(changedLineBlocks('a\nb\n', 'a\nb\n'), []);
    });

    test('groups consecutive changed lines into one block', () => {
        assert.deepStrictEqual(changedLineBlocks('a\nb\nc\nd', 'a\nB\nC\nd'), [[1, 2]]);
    });

    test('separates blocks that are apart', () => {
        assert.deepStrictEqual(
            changedLineBlocks('a\nb\nc\nd\ne', 'A\nb\nc\nd\nE'),
            [[0, 0], [4, 4]]
        );
    });

    test('marks the seam when formatting only inserts lines', () => {
        assert.deepStrictEqual(changedLineBlocks('a\nb', 'a\nx\nb'), [[1, 1]]);
    });

    test('reports deleted lines against the original', () => {
        assert.deepStrictEqual(changedLineBlocks('a\nx\nb', 'a\nb'), [[1, 1]]);
    });
});
