import * as vscode from 'vscode';

export type ControlCenterActionId =
    | 'runWorkspace'
    | 'runChanged'
    | 'runStaged'
    | 'setDefault'
    | 'openLogs'
    | 'reloadWorkspace';

interface ControlCenterItem extends vscode.QuickPickItem {
    id: ControlCenterActionId;
}

interface TooltipCommands {
    openLogs: string;
    openControlCenter: string;
    formatWorkspace: string;
    formatChanged: string;
    formatStaged: string;
    setDefaultFormatter: string;
    reloadWorkspace: string;
}

export function buildStatusBarTooltip(extensionVersion: string, commands: TooltipCommands): vscode.MarkdownString {
    const tooltip = new vscode.MarkdownString('', true);
    tooltip.isTrusted = true;
    tooltip.appendMarkdown(`rust-fmt: Version ${extensionVersion}\n\n`);
    tooltip.appendMarkdown(`[Open Logs](command:${commands.openLogs})\n\n`);
    tooltip.appendMarkdown(`[Open Control Center](command:${commands.openControlCenter})\n\n`);
    tooltip.appendMarkdown(`[Format Workspace](command:${commands.formatWorkspace})\n\n`);
    tooltip.appendMarkdown(`[Format Changed Rust Files](command:${commands.formatChanged})\n\n`);
    tooltip.appendMarkdown(`[Format Staged Rust Files](command:${commands.formatStaged})\n\n`);
    tooltip.appendMarkdown(`[Set as Default Formatter](command:${commands.setDefaultFormatter})\n\n`);
    tooltip.appendMarkdown(`[Reload Workspace](command:${commands.reloadWorkspace})`);
    return tooltip;
}

export async function pickControlCenterAction(): Promise<ControlCenterActionId | undefined> {
    const items: Array<vscode.QuickPickItem | ControlCenterItem> = [
        { label: 'Actions', kind: vscode.QuickPickItemKind.Separator },
        {
            id: 'runWorkspace',
            label: 'Run: Format Workspace',
            description: 'Format all Rust files in workspace'
        },
        {
            id: 'runChanged',
            label: 'Run: Format Changed Rust Files',
            description: 'Use git diff working tree'
        },
        {
            id: 'runStaged',
            label: 'Run: Format Staged Rust Files',
            description: 'Use git diff --cached'
        },
        { id: 'setDefault', label: 'Run: Set rust-fmt as Default Formatter' },
        { label: 'Maintenance', kind: vscode.QuickPickItemKind.Separator },
        { id: 'openLogs', label: 'Open Logs' },
        { id: 'reloadWorkspace', label: 'Reload Workspace' }
    ];

    const selection = await vscode.window.showQuickPick(items, {
        placeHolder: 'rust-fmt Control Center'
    });

    if (!selection || !('id' in selection)) {
        return undefined;
    }

    return selection.id;
}
