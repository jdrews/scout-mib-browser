import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

// ── State ────────────────────────────────────────────────────────

/** @type {{ oid: string; name: string; syntaxType?: string; mibName: string } | null} */
let selectedNode = null;

/** @type {Array<{oid: string; name: string; children?: Array<any>; syntax_type?: string; mib_name: string}>} */
let treeData = [];

/** Context menu target node data */
let contextMenuTarget = null;

// ── DOM References ───────────────────────────────────────────────

const fileMenuTrigger = document.getElementById("file-menu-trigger");
const fileMenu = document.getElementById("file-menu");
const treeContainer = document.getElementById("tree-container");
const addressBar = document.getElementById("oid-address-bar");
const goButton = document.getElementById("go-button");
const autocompleteDropdown = document.getElementById("autocomplete-dropdown");
const selectionInfo = document.getElementById("selection-info");
const selectionName = document.getElementById("selection-name");
const selectionSyntax = document.getElementById("selection-syntax");
const selectionMib = document.getElementById("selection-mib");
const resultsContent = document.getElementById("results-content");
const contextMenu = document.getElementById("context-menu");
const fallbackBanner = document.getElementById("fallback-banner");
const fallbackMessage = document.getElementById("fallback-message");
const statusText = document.getElementById("status-text");
const nodeCountEl = document.getElementById("node-count");
const manageMibsOverlay = document.getElementById("manage-mibs-overlay");
const manageMibsList = document.getElementById("manage-mibs-list");

// ── Autocomplete State ───────────────────────────────────────────

let autocompleteTimer = null;
let highlightedIndex = -1;

// ── Initialization ───────────────────────────────────────────────

async function init() {
    setupMenu();
    setupAddressBar();
    setupContextMenu();
    setupManageMibsDialog();
    await loadConfigAndMibs();
}

/** Loads config, reads MIB directories, and populates the tree. */
async function loadConfigAndMibs() {
    setStatus("Loading configuration...");
    try {
        const config = await invoke("config_read");
        const dirs = config.mib?.directories || [];

        if (dirs.length > 0) {
            setStatus(`Loading MIBs from ${dirs.length} directory(ies)...`);
            const status = await invoke("mib_load_directories", { directories: dirs });
            updateNodeCount(status.nodeCount);
            showFallbackBanner(status.fallbackMibs);
        }

        await refreshTree();
        setStatus("Ready");
    } catch (err) {
        setStatus(`Error: ${err}`);
        console.error("Failed to load MIBs:", err);
    }
}

/** Refreshes the tree view from backend data. */
async function refreshTree() {
    try {
        treeData = await invoke("mib_tree");
        renderTree();
    } catch (err) {
        console.error("Failed to load tree:", err);
    }
}

// ── Menu ─────────────────────────────────────────────────────────

function setupMenu() {
    fileMenuTrigger.addEventListener("click", (e) => {
        e.stopPropagation();
        toggleMenu(fileMenu, fileMenuTrigger);
    });

    document.addEventListener("click", () => closeAllMenus());

    // Menu actions
    fileMenu.querySelectorAll(".menu-action").forEach((el) => {
        el.addEventListener("click", async (e) => {
            const action = e.currentTarget.dataset.action;
            closeAllMenus();
            await handleMenuAction(action);
        });
    });
}

function toggleMenu(menu, trigger) {
    const isVisible = menu.classList.contains("visible");
    closeAllMenus();
    if (!isVisible) {
        menu.classList.add("visible");
        trigger.classList.add("active");
    }
}

function closeAllMenus() {
    fileMenu.classList.remove("visible");
    fileMenuTrigger.classList.remove("active");
}

async function handleMenuAction(action) {
    switch (action) {
        case "add-mib-directory":
            await addMibDirectory();
            break;
        case "manage-mibs":
            showManageMibsDialog();
            break;
    }
}

/** Opens native folder picker and adds selected directory to MIB paths. */
async function addMibDirectory() {
    try {
        const selected = await open({ directory: true, multiple: false });
        if (!selected) return;

        setStatus("Loading MIBs...");

        // Read current config, append new directory, persist.
        const config = await invoke("config_read");
        const dirs = config.mib?.directories || [];
        if (!dirs.includes(selected)) {
            dirs.push(selected);
            await invoke("config_write", {
                path: "mib.directories",
                value: dirs,
            });
        }

        // Reload MIBs from all directories.
        const status = await invoke("mib_load_directories", { directories: dirs });
        updateNodeCount(status.nodeCount);
        showFallbackBanner(status.fallbackMibs);
        await refreshTree();
        setStatus(`Loaded ${status.nodeCount} nodes`);
    } catch (err) {
        setStatus(`Error: ${err}`);
        console.error("Failed to add MIB directory:", err);
    }
}

// ── Tree Rendering ───────────────────────────────────────────────

function renderTree() {
    treeContainer.innerHTML = "";
    if (treeData.length === 0) {
        treeContainer.innerHTML =
            '<p class="placeholder-text">No MIBs loaded.\nUse File → Add MIB Directory to get started.</p>';
        return;
    }

    for (const node of treeData) {
        const el = buildTreeNode(node);
        treeContainer.appendChild(el);
    }
}

/** Builds a DOM element for a single tree node. */
function buildTreeNode(node) {
    const fragment = document.createDocumentFragment();

    // Row
    const row = document.createElement("div");
    row.className = "tree-row";
    row.dataset.oid = node.oid;
    row.dataset.name = node.name;
    if (node.mib_name) row.dataset.mibName = node.mib_name;
    if (node.syntax_type) row.dataset.syntaxType = node.syntax_type;

    // Toggle arrow
    const toggle = document.createElement("span");
    toggle.className = "tree-toggle";
    const hasChildren = node.children && node.children.length > 0;
    toggle.textContent = hasChildren ? "▶" : "";
    if (hasChildren) {
        toggle.addEventListener("click", (e) => {
            e.stopPropagation();
            toggleExpand(row, childrenContainer, toggle);
        });
    }

    // Icon
    const icon = document.createElement("span");
    icon.className = "tree-icon";
    icon.textContent = hasChildren ? "📁" : "📄";

    // Label
    const label = document.createElement("span");
    label.className = "tree-label";
    label.textContent = node.name;
    label.title = `${node.name} (${node.oid})`;

    // OID (right-aligned)
    const oidSpan = document.createElement("span");
    oidSpan.className = "tree-oid";
    oidSpan.textContent = node.oid;

    row.appendChild(toggle);
    row.appendChild(icon);
    row.appendChild(label);
    row.appendChild(oidSpan);

    // Selection handler
    row.addEventListener("click", () => selectNode(row, node));

    // Context menu handler
    row.addEventListener("contextmenu", (e) => {
        e.preventDefault();
        showContextMenu(e.clientX, e.clientY, node);
    });

    fragment.appendChild(row);

    // Children container
    if (hasChildren) {
        const childrenContainer = document.createElement("div");
        childrenContainer.className = "tree-children";
        for (const child of node.children) {
            childrenContainer.appendChild(buildTreeNode(child));
        }
        fragment.appendChild(childrenContainer);
    }

    return fragment;
}

/** Toggles expand/collapse state of a tree row. */
function toggleExpand(row, childrenEl, toggle) {
    const isExpanded = childrenEl.classList.contains("expanded");
    if (isExpanded) {
        childrenEl.classList.remove("expanded");
        toggle.textContent = "▶";
    } else {
        childrenEl.classList.add("expanded");
        toggle.textContent = "▼";
    }
}

/** Handles node selection: updates address bar and selection info. */
function selectNode(row, node) {
    // Clear previous selection
    treeContainer.querySelectorAll(".tree-row.selected").forEach((el) => {
        el.classList.remove("selected");
    });
    row.classList.add("selected");

    selectedNode = node;

    // Update address bar (bidirectional binding — suppress change event).
    const suppressChange = true;
    addressBar.value = `${node.oid}  ${node.name}`;

    // Show selection info
    updateSelectionInfo(node);
}

function updateSelectionInfo(node) {
    if (!node) {
        selectionInfo.classList.add("hidden");
        return;
    }
    selectionInfo.classList.remove("hidden");
    selectionName.textContent = node.name;
    selectionSyntax.textContent = node.syntax_type || "OID";
    selectionMib.textContent = node.mib_name || "";
}

// ── Address Bar & Autocomplete ───────────────────────────────────

function setupAddressBar() {
    let lastSelectedValue = addressBar.value;

    addressBar.addEventListener("input", () => {
        const val = addressBar.value.trim();
        if (val.length < 1) {
            hideAutocomplete();
            return;
        }
        highlightedIndex = -1;

        // Debounce search
        clearTimeout(autocompleteTimer);
        autocompleteTimer = setTimeout(() => {
            performSearch(val);
        }, 150);
    });

    addressBar.addEventListener("keydown", (e) => {
        const items = autocompleteDropdown.querySelectorAll(".autocomplete-item");
        if (!items.length) return;

        if (e.key === "ArrowDown") {
            e.preventDefault();
            highlightedIndex = Math.min(highlightedIndex + 1, items.length - 1);
            updateHighlight(items);
        } else if (e.key === "ArrowUp") {
            e.preventDefault();
            highlightedIndex = Math.max(highlightedIndex - 1, 0);
            updateHighlight(items);
        } else if (e.key === "Enter" && highlightedIndex >= 0) {
            e.preventDefault();
            const item = items[highlightedIndex];
            selectAutocompleteItem(item);
        } else if (e.key === "Escape") {
            hideAutocomplete();
        }
    });

    goButton.addEventListener("click", () => handleGo());

    addressBar.addEventListener("keydown", (e) => {
        if (e.key === "Enter") {
            e.preventDefault();
            handleGo();
        }
    });

    // Close autocomplete on outside click.
    document.addEventListener("click", (e) => {
        if (!e.target.closest("#address-bar-container")) {
            hideAutocomplete();
        }
    });
}

async function performSearch(query) {
    try {
        const results = await invoke("mib_search", { query });
        renderAutocomplete(results);
    } catch (err) {
        console.error("Search failed:", err);
    }
}

function renderAutocomplete(items) {
    autocompleteDropdown.innerHTML = "";
    if (!items || items.length === 0) {
        hideAutocomplete();
        return;
    }

    for (const item of items) {
        const el = document.createElement("div");
        el.className = "autocomplete-item";
        el.dataset.oid = item.oid;
        el.dataset.name = item.name;

        const nameSpan = document.createElement("span");
        nameSpan.className = "autocomplete-name";
        nameSpan.textContent = item.name;

        const oidSpan = document.createElement("span");
        oidSpan.className = "autocomplete-oid";
        oidSpan.textContent = item.oid;

        el.appendChild(nameSpan);
        el.appendChild(oidSpan);

        el.addEventListener("click", () => selectAutocompleteItem(el));
        autocompleteDropdown.appendChild(el);
    }

    autocompleteDropdown.classList.remove("hidden");
}

function updateHighlight(items) {
    items.forEach((el, i) => {
        el.classList.toggle("highlighted", i === highlightedIndex);
    });
    if (items[highlightedIndex]) {
        items[highlightedIndex].scrollIntoView({ block: "nearest" });
    }
}

function selectAutocompleteItem(el) {
    const oid = el.dataset.oid;
    const name = el.dataset.name;
    addressBar.value = `${oid}  ${name}`;
    hideAutocomplete();

    // Try to find and select the node in the tree.
    trySelectInTree(oid);
}

/** Tries to find a node by OID in the rendered tree and selects it. */
function trySelectInTree(oid) {
    const row = treeContainer.querySelector(`.tree-row[data-oid="${cssEscape(oid)}"]`);
    if (row) {
        // Expand parents first.
        let parent = row.parentElement;
        while (parent && parent !== treeContainer) {
            if (parent.classList.contains("tree-children")) {
                parent.classList.add("expanded");
                const prevToggle = parent.previousElementSibling?.querySelector(".tree-toggle");
                if (prevToggle) prevToggle.textContent = "▼";
            }
            parent = parent.parentElement;
        }

        // Simulate click to trigger selection.
        row.click();
    }
}

function hideAutocomplete() {
    autocompleteDropdown.classList.add("hidden");
    highlightedIndex = -1;
}

/** Handles the Go button: executes SNMP operation or navigates tree. */
async function handleGo() {
    const val = addressBar.value.trim();
    if (!val) return;

    // Parse OID from address bar (format: "OID  Name" or just "OID").
    let oid = val.split(/\s{2,}/)[0].trim();
    if (!oid) oid = val.trim();

    setStatus(`Executing operation for ${oid}...`);

    // If the OID is in our loaded MIB tree, navigate to it.
    const foundRow = treeContainer.querySelector(`.tree-row[data-oid="${cssEscape(oid)}"]`);
    if (foundRow) {
        foundRow.click();
        setStatus("Navigated to selected node");
        return;
    }

    // Otherwise, this is a raw OID — could execute SNMP operation here.
    // For now, just show in results that it's not in loaded MIBs.
    resultsContent.innerHTML = `
        <div class="placeholder-text">
            OID ${escapeHtml(oid)} is not in the loaded MIB tree.<br>
            Configure a Target and execute an SNMP operation to query it directly.
        </div>`;
    setStatus("Ready");
}

// ── Context Menu ─────────────────────────────────────────────────

function setupContextMenu() {
    contextMenu.querySelectorAll(".menu-action").forEach((el) => {
        el.addEventListener("click", async () => {
            const action = el.dataset.action;
            hideContextMenu();
            await handleContextAction(action);
        });
    });

    document.addEventListener("click", hideContextMenu);
    document.addEventListener("contextmenu", (e) => {
        if (!e.target.closest(".tree-row")) {
            hideContextMenu();
        }
    });
}

function showContextMenu(x, y, node) {
    contextMenuTarget = node;
    contextMenu.style.left = `${x}px`;
    contextMenu.style.top = `${y}px`;
    contextMenu.classList.remove("hidden");

    // Ensure menu stays within viewport.
    requestAnimationFrame(() => {
        const rect = contextMenu.getBoundingClientRect();
        if (rect.right > window.innerWidth) {
            contextMenu.style.left = `${x - rect.width}px`;
        }
        if (rect.bottom > window.innerHeight) {
            contextMenu.style.top = `${y - rect.height}px`;
        }
    });
}

function hideContextMenu() {
    contextMenu.classList.add("hidden");
    contextMenuTarget = null;
}

async function handleContextAction(action) {
    if (!contextMenuTarget) return;

    try {
        switch (action) {
            case "copy-oid":
                await navigator.clipboard.writeText(contextMenuTarget.oid);
                setStatus(`Copied OID: ${contextMenuTarget.oid}`);
                break;
            case "copy-name":
                await navigator.clipboard.writeText(contextMenuTarget.name);
                setStatus(`Copied name: ${contextMenuTarget.name}`);
                break;
        }
    } catch (err) {
        console.error("Clipboard error:", err);
        setStatus("Failed to copy");
    }
}

// ── Manage MIBs Dialog ───────────────────────────────────────────

function setupManageMibsDialog() {
    // Close buttons
    document.querySelectorAll("[data-action='close-manage-mibs']").forEach((el) => {
        el.addEventListener("click", hideManageMibsDialog);
    });

    manageMibsOverlay.addEventListener("click", (e) => {
        if (e.target === manageMibsOverlay) {
            hideManageMibsDialog();
        }
    });
}

async function showManageMibsDialog() {
    try {
        const mibs = await invoke("mib_loaded_list");
        renderManageMibsList(mibs);
        manageMibsOverlay.classList.remove("hidden");
    } catch (err) {
        setStatus(`Error: ${err}`);
        console.error("Failed to load MIB list:", err);
    }
}

function hideManageMibsDialog() {
    manageMibsOverlay.classList.add("hidden");
}

function renderManageMibsList(mibs) {
    manageMibsList.innerHTML = "";

    if (!mibs || mibs.length === 0) {
        manageMibsList.innerHTML =
            '<p class="placeholder-text">No MIBs currently loaded.</p>';
        return;
    }

    for (const mib of mibs) {
        const entry = document.createElement("div");
        entry.className = "mib-entry";

        // Name
        const nameEl = document.createElement("span");
        nameEl.className = "mib-entry-name";
        nameEl.textContent = mib.mibName;

        // Path
        const pathEl = document.createElement("span");
        pathEl.className = "mib-entry-path";
        pathEl.textContent = mib.filePath;
        pathEl.title = mib.filePath;

        // Meta
        const metaEl = document.createElement("span");
        metaEl.className = "mib-entry-meta";

        if (mib.isFallback) {
            const tag = document.createElement("span");
            tag.className = "fallback-tag";
            tag.textContent = "FALLBACK";
            metaEl.appendChild(tag);
        }

        // Node count
        const countSpan = document.createElement("span");
        countSpan.textContent = `${mib.nodeCount} nodes`;
        metaEl.appendChild(countSpan);

        // Unload button
        const unloadBtn = document.createElement("button");
        unloadBtn.className = "unload-btn";
        unloadBtn.textContent = "Unload";
        unloadBtn.addEventListener("click", async () => {
            await unloadMibModule(mib.mibName, entry);
        });

        entry.appendChild(nameEl);
        entry.appendChild(pathEl);
        entry.appendChild(metaEl);
        entry.appendChild(unloadBtn);
        manageMibsList.appendChild(entry);
    }
}

/** Unloads a MIB module and refreshes the UI. */
async function unloadMibModule(mibName, domEntry) {
    try {
        const status = await invoke("mib_unload", { mibName });
        updateNodeCount(status.nodeCount);
        showFallbackBanner(status.fallbackMibs);
        await refreshTree();

        // Remove from dialog list.
        if (domEntry) domEntry.remove();

        setStatus(`Unloaded ${mibName}`);
    } catch (err) {
        setStatus(`Error: ${err}`);
        console.error("Failed to unload MIB:", err);
    }
}

// ── Fallback Banner ──────────────────────────────────────────────

function showFallbackBanner(fallbackMibs) {
    if (!fallbackMibs || fallbackMibs.length === 0) {
        fallbackBanner.classList.add("hidden");
        return;
    }

    fallbackMessage.textContent = `${fallbackMibs.length} MIB(s) loaded via regex fallback`;
    fallbackBanner.classList.remove("hidden");
}

// ── Status Bar ───────────────────────────────────────────────────

function setStatus(text) {
    statusText.textContent = text;
}

function updateNodeCount(count) {
    nodeCountEl.textContent = count ? `${count} nodes loaded` : "";
}

// ── Utilities ────────────────────────────────────────────────────

function escapeHtml(str) {
    const div = document.createElement("div");
    div.textContent = str;
    return div.innerHTML;
}

function cssEscape(str) {
    return str.replace(/"/g, '\\"').replace(/'/g, "\\'");
}

// ── Boot ─────────────────────────────────────────────────────────

init();
