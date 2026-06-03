import * as vscode from 'vscode';

export const DEFAULT_FORMATTER_ID = 'vremyavnikuda.rust-fmt';
export const DEFAULT_FORMATTER_COMMAND = 'rust-fmt.useAsDefaultFormatter';

const DEFAULT_FORMATTER_LAST_OBSERVED_KEY = 'rustfmt.defaultFormatterLastObserved';
const DEFAULT_FORMATTER_PROMPT_SUPPRESS_KEY = 'rustfmt.defaultFormatterPromptSuppress';
const DEFAULT_FORMATTER_NONE_SENTINEL = '__none__';
const DEFAULT_FORMATTER_PROMPT_STATE_VERSION_KEY = 'rustfmt.defaultFormatterPromptStateVersion';
const DEFAULT_FORMATTER_PROMPT_STATE_VERSION = 1;
const ONBOARDING_MODE_GUIDED = 'guided';
let promptInProgress = false;
let ignoreLastObservedOnce = false;

export function initializeDefaultFormatterPromptState(context: vscode.ExtensionContext): void {
    const promptStateVersion = context.workspaceState.get<number>(DEFAULT_FORMATTER_PROMPT_STATE_VERSION_KEY);
    if (promptStateVersion !== DEFAULT_FORMATTER_PROMPT_STATE_VERSION) {
        ignoreLastObservedOnce = true;
        void context.workspaceState.update(DEFAULT_FORMATTER_PROMPT_STATE_VERSION_KEY, DEFAULT_FORMATTER_PROMPT_STATE_VERSION);
        void context.workspaceState.update(DEFAULT_FORMATTER_LAST_OBSERVED_KEY, undefined);
    }
}

export async function maybePromptDefaultFormatter(
    context: vscode.ExtensionContext,
    editor?: vscode.TextEditor | null
): Promise<void> {
    if (!editor || editor.document.languageId !== 'rust') {
        return;
    }
    if (getOnboardingMode(editor.document.uri) !== ONBOARDING_MODE_GUIDED) {
        return;
    }
    if (context.globalState.get(DEFAULT_FORMATTER_PROMPT_SUPPRESS_KEY)) {
        return;
    }
    const currentDefaultFormatter = getCurrentDefaultFormatter(editor.document);
    const currentKey = currentDefaultFormatter ?? DEFAULT_FORMATTER_NONE_SENTINEL;
    let lastObserved = context.workspaceState.get<string>(DEFAULT_FORMATTER_LAST_OBSERVED_KEY);
    if (ignoreLastObservedOnce) {
        lastObserved = undefined;
        ignoreLastObservedOnce = false;
    }
    if (currentDefaultFormatter === DEFAULT_FORMATTER_ID) {
        await context.workspaceState.update(DEFAULT_FORMATTER_LAST_OBSERVED_KEY, currentKey);
        return;
    }
    if (promptInProgress) {
        return;
    }
    if (lastObserved === currentKey) {
        return;
    }
    await context.workspaceState.update(DEFAULT_FORMATTER_LAST_OBSERVED_KEY, currentKey);
    promptInProgress = true;
    try {
        let message = 'No default formatter is configured for Rust. Switch to rust-fmt?';
        if (currentDefaultFormatter) {
            const otherFormatterLabel = resolveFormatterLabel(currentDefaultFormatter);
            message = otherFormatterLabel
                ? `Rust is currently formatted by "${otherFormatterLabel}". Switch to rust-fmt?`
                : 'A different default formatter is set for Rust. Switch to rust-fmt?';
        }
        const choice = await vscode.window.showInformationMessage(
            message,
            'Switch',
            "Don't ask again"
        );
        if (choice === 'Switch') {
            await applyDefaultFormatterSettings(editor.document.uri, context, { askTarget: true });
        } else if (choice === "Don't ask again") {
            await context.globalState.update(DEFAULT_FORMATTER_PROMPT_SUPPRESS_KEY, true);
        }
    } finally {
        promptInProgress = false;
    }
}

export function getRustfmtConfiguration(resource?: vscode.Uri): vscode.WorkspaceConfiguration {
    return vscode.workspace.getConfiguration('rustfmt', getPreferredResource(resource));
}

export function getRustConfigurationScope(resource?: vscode.Uri): vscode.ConfigurationScope {
    if (resource) {
        return { uri: resource, languageId: 'rust' };
    }
    return { languageId: 'rust' };
}

export async function applyDefaultFormatterSettings(
    resource?: vscode.Uri,
    context?: vscode.ExtensionContext,
    options?: { askTarget?: boolean; target?: vscode.ConfigurationTarget }
): Promise<void> {
    const scope = getRustConfigurationScope(resource);
    const config = vscode.workspace.getConfiguration('editor', scope);
    let target = options?.target;
    if (options?.askTarget) {
        target = await pickConfigurationTarget();
        if (target === undefined) {
            return;
        }
    }
    if (target === undefined) {
        const fallbackTarget = getConfigurationTarget(resource);
        target = resolveConfigurationTarget(config, fallbackTarget);
    }
    try {
        await config.update('defaultFormatter', DEFAULT_FORMATTER_ID, target, true);
        await config.update('formatOnSave', true, target, true);
        if (context) {
            await context.workspaceState.update(DEFAULT_FORMATTER_LAST_OBSERVED_KEY, DEFAULT_FORMATTER_ID);
        }
    } catch (err) {
        console.error('[rust-fmt] Failed to update formatter settings', err);
    }
}

function getCurrentDefaultFormatter(document: vscode.TextDocument): string | undefined {
    const config = vscode.workspace.getConfiguration('editor', getRustConfigurationScope(document.uri));
    return config.get<string>('defaultFormatter') || undefined;
}

function getOnboardingMode(resource?: vscode.Uri): 'quiet' | 'guided' {
    const config = getRustfmtConfiguration(resource);
    const mode = config.get<string>('onboarding.mode');
    return mode === ONBOARDING_MODE_GUIDED ? 'guided' : 'quiet';
}

function getPreferredResource(resource?: vscode.Uri): vscode.Uri | undefined {
    if (resource) {
        return resource;
    }
    const activeUri = vscode.window.activeTextEditor?.document.uri;
    if (activeUri) {
        return activeUri;
    }
    return vscode.workspace.workspaceFolders?.[0]?.uri;
}

function resolveFormatterLabel(extensionId: string): string | undefined {
    const extension = vscode.extensions.getExtension(extensionId);
    if (!extension) {
        return extensionId;
    }
    const packageJson = extension.packageJSON ?? {};
    return packageJson.displayName || packageJson.name || extensionId;
}

function getConfigurationTarget(resource?: vscode.Uri): vscode.ConfigurationTarget {
    const folders = vscode.workspace.workspaceFolders;
    if (!folders || folders.length === 0) {
        return vscode.ConfigurationTarget.Global;
    }
    if (resource) {
        const folder = vscode.workspace.getWorkspaceFolder(resource);
        if (folder) {
            return vscode.ConfigurationTarget.WorkspaceFolder;
        }
    }
    return vscode.ConfigurationTarget.Workspace;
}

function resolveConfigurationTarget(
    config: vscode.WorkspaceConfiguration,
    fallback: vscode.ConfigurationTarget
): vscode.ConfigurationTarget {
    const inspected = config.inspect<string>('defaultFormatter');
    if (!inspected) {
        return fallback;
    }
    if (
        inspected.workspaceFolderLanguageValue !== undefined ||
        inspected.workspaceFolderValue !== undefined
    ) {
        return vscode.ConfigurationTarget.WorkspaceFolder;
    }
    if (
        inspected.workspaceLanguageValue !== undefined ||
        inspected.workspaceValue !== undefined
    ) {
        return vscode.ConfigurationTarget.Workspace;
    }
    if (
        inspected.globalLanguageValue !== undefined ||
        inspected.globalValue !== undefined
    ) {
        return vscode.ConfigurationTarget.Global;
    }
    return fallback;
}

async function pickConfigurationTarget(): Promise<vscode.ConfigurationTarget | undefined> {
    const folders = vscode.workspace.workspaceFolders;
    if (!folders || folders.length === 0) {
        return vscode.ConfigurationTarget.Global;
    }
    const selection = await vscode.window.showQuickPick(
        [
            { label: 'Global', description: 'Apply to all workspaces' },
            { label: 'Workspace', description: 'Apply to this workspace only' }
        ],
        {
            placeHolder: 'Set rust-fmt as default formatter for Rust',
            canPickMany: false
        }
    );
    if (!selection) {
        return undefined;
    }
    return selection.label === 'Workspace'
        ? vscode.ConfigurationTarget.Workspace
        : vscode.ConfigurationTarget.Global;
}
