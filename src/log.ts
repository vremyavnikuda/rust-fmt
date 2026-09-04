import * as vscode from 'vscode';

let channel: vscode.OutputChannel | undefined;

/// The single "rust-fmt" output channel, and what the Open Logs command shows.
///
/// A plain channel, deliberately: a LogOutputChannel hides everything below
/// the window's log level, so the lines you need when a format goes wrong are
/// missing exactly when you go looking for them.
export function logChannel(): vscode.OutputChannel {
    channel ??= vscode.window.createOutputChannel('rust-fmt');
    return channel;
}

function write(level: string, message: unknown, rest: unknown[]): void {
    const extra = rest.length > 0 ? ` ${rest.map((value) => String(value)).join(' ')}` : '';
    logChannel().appendLine(`[${new Date().toISOString()}] ${level} ${String(message)}${extra}`);
}

export function logger() {
    return {
        debug: (message: unknown, ...rest: unknown[]) => write('debug', message, rest),
        info: (message: unknown, ...rest: unknown[]) => write('info', message, rest),
        warn: (message: unknown, ...rest: unknown[]) => write('warn', message, rest),
        error: (message: unknown, ...rest: unknown[]) => write('error', message, rest),
        show: (preserveFocus?: boolean) => logChannel().show(preserveFocus)
    };
}
