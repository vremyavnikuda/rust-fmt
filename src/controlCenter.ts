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
