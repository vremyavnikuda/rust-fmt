/**
 * One run of lines that formatting replaces: `original` lines
 * `[originalStart, originalEnd)` become `formatted` lines
 * `[formattedStart, formattedEnd)`. An empty original side is an insertion
 * before `originalStart`.
 */
export interface LineHunk {
    originalStart: number;
    originalEnd: number;
    formattedStart: number;
    formattedEnd: number;
}

/**
 * The line-level differences between `original` and `formatted`, in order.
 *
 * ponytail: common prefix/suffix trim, then an O(n*m) LCS over the differing
 * middle. Above the cap the whole middle becomes one hunk — swap in a Myers
 * diff if a real file ever trips it.
 */
export function lineHunks(original: string, formatted: string): LineHunk[] {
    if (original === formatted) {
        return [];
    }
    const a = original.split('\n');
    const b = formatted.split('\n');
    let head = 0;
    while (head < a.length && head < b.length && a[head] === b[head]) {
        head += 1;
    }
    let tail = 0;
    while (
        tail < a.length - head &&
        tail < b.length - head &&
        a[a.length - 1 - tail] === b[b.length - 1 - tail]
    ) {
        tail += 1;
    }
    const left = a.slice(head, a.length - tail);
    const right = b.slice(head, b.length - tail);
    const n = left.length;
    const m = right.length;
    if (n === 0 || m === 0 || n * m > 4_000_000) {
        return [{ originalStart: head, originalEnd: head + n, formattedStart: head, formattedEnd: head + m }];
    }
    const width = m + 1;
    const table = new Int32Array((n + 1) * width);
    for (let i = n - 1; i >= 0; i -= 1) {
        for (let j = m - 1; j >= 0; j -= 1) {
            table[i * width + j] = left[i] === right[j]
                ? table[(i + 1) * width + j + 1] + 1
                : Math.max(table[(i + 1) * width + j], table[i * width + j + 1]);
        }
    }
    const hunks: LineHunk[] = [];
    let open: LineHunk | undefined;
    const close = (i: number, j: number) => {
        if (open) {
            open.originalEnd = head + i;
            open.formattedEnd = head + j;
            hunks.push(open);
            open = undefined;
        }
    };
    let i = 0;
    let j = 0;
    while (i < n || j < m) {
        if (i < n && j < m && left[i] === right[j]) {
            close(i, j);
            i += 1;
            j += 1;
            continue;
        }
        open ??= { originalStart: head + i, originalEnd: 0, formattedStart: head + j, formattedEnd: 0 };
        if (j >= m || (i < n && table[(i + 1) * width + j] >= table[i * width + j + 1])) {
            i += 1;
        } else {
            j += 1;
        }
    }
    close(i, j);
    return hunks;
}

/**
 * Contiguous `[startLine, endLine]` ranges of `original` that formatting would
 * change, as 0-based inclusive line indexes.
 */
export function changedLineBlocks(original: string, formatted: string): Array<[number, number]> {
    const hunks = lineHunks(original, formatted);
    if (hunks.length === 0) {
        return [];
    }
    const blocks = hunks
        .filter((hunk) => hunk.originalEnd > hunk.originalStart)
        .map((hunk): [number, number] => [hunk.originalStart, hunk.originalEnd - 1]);
    if (blocks.length > 0) {
        return blocks;
    }
    // Formatting only inserted lines: nothing in the original changed, so point
    // at the seam instead of reporting a clean file.
    const lastLine = original.split('\n').length - 1;
    const seam = Math.min(hunks[0].originalStart, lastLine);
    return [[seam, seam]];
}

/**
 * `original` with only those formatting changes applied that touch lines
 * `startLine..=endLine` of it. A change is taken whole, so one that crosses
 * the edge of the selection is applied entirely; an insertion counts when it
 * lands inside the selection or right after it.
 */
export function applyHunksInLines(
    original: string,
    formatted: string,
    startLine: number,
    endLine: number
): string {
    const a = original.split('\n');
    const b = formatted.split('\n');
    const result: string[] = [];
    let position = 0;
    for (const hunk of lineHunks(original, formatted)) {
        const touches = hunk.originalEnd > hunk.originalStart
            ? hunk.originalStart <= endLine && hunk.originalEnd - 1 >= startLine
            : hunk.originalStart >= startLine && hunk.originalStart <= endLine + 1;
        if (!touches) {
            continue;
        }
        result.push(...a.slice(position, hunk.originalStart));
        result.push(...b.slice(hunk.formattedStart, hunk.formattedEnd));
        position = hunk.originalEnd;
    }
    result.push(...a.slice(position));
    return result.join('\n');
}
