import * as vscode from 'vscode';

let channel: vscode.LogOutputChannel | undefined;

/// The single "rust-fmt" output channel, and what the Open Logs command
/// shows. A LogOutputChannel so verbosity is the user's choice through VS
/// Code's own log-level picker instead of a setting of ours: the
/// per-format chatter is `debug`, notable events are `info`.
export function logger(): vscode.LogOutputChannel {
    channel ??= vscode.window.createOutputChannel('rust-fmt', { log: true });
    return channel;
}
