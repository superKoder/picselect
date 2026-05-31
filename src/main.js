const { invoke } = window.__TAURI__.core;
const { open: selectDirectory, save: saveFileDialog } = window.__TAURI__.dialog;

// App State
let mediaItems = [];
let currentIndex = -1;
let activeConfig = null;
let projectFilePath = null;
let currentRotation = 0; // Visual rotation for current item
let isLivePlayback = false;
let autoHideTimeout = null;
// UI Elements
let onboardingScreen, mainScreen;
let btnSelectProject, btnBrowseIntake, btnBrowseSelected, btnBrowseDeleted, btnBrowseProjectFile, btnCreateProject;
let tabOpen, tabNew, contentOpen, contentNew;
let recentProjectsList, newIntakeDir, newSelectedDir, newDeletedDir, newProjectFile, newCacheLimit, cacheLimitVal;

let mediaImage, mediaVideo, mediaLiveVideo, mediaBackdrop;
let badgeLive, badgeVideo, preloadProgress, controlBar;
let mediaFilename, mediaIndex, statsBubble, preloaderIndicator, actionOverlay;

let btnPrev, btnNext, btnDelete, btnSelect, btnUndo, btnRotate, btnSaveRotate, btnRename, btnSettings;

let renameModal, renameInput, renameError, btnRenameCancel, btnRenameConfirm;
let settingsModal, settingsSelectedDir, settingsDeletedDir, settingsCacheLimit, settingsCacheLimitVal, btnSettingsCancel, btnSettingsSave;
let btnSettingsBrowseSelected, btnSettingsBrowseDeleted;
let toast, toastText;

// Initialization
window.addEventListener("DOMContentLoaded", async () => {
  bindUIElements();
  setupTabSystem();
  setupOnboardingActions();
  setupMainViewerActions();
  setupKeyboardShortcuts();
  setupAutoHideControls();
  
  // Load recents on start
  await loadRecentProjects();
  
  // Periodically refresh cache stats
  setInterval(updateCacheStats, 3000);

  // Prevent buttons from retaining focus and stealing keyboard shortcuts
  document.querySelectorAll("button").forEach(btn => {
    btn.addEventListener("click", () => {
      btn.blur();
    });
  });
});

function bindUIElements() {
  onboardingScreen = document.getElementById("onboarding-screen");
  mainScreen = document.getElementById("main-screen");
  
  // Tabs
  tabOpen = document.getElementById("tab-open");
  tabNew = document.getElementById("tab-new");
  contentOpen = document.getElementById("content-open");
  contentNew = document.getElementById("content-new");
  
  // Onboarding controls
  btnSelectProject = document.getElementById("btn-select-project");
  btnBrowseIntake = document.getElementById("btn-browse-intake");
  btnBrowseSelected = document.getElementById("btn-browse-selected");
  btnBrowseDeleted = document.getElementById("btn-browse-deleted");
  btnBrowseProjectFile = document.getElementById("btn-browse-project-file");
  btnCreateProject = document.getElementById("btn-create-project");
  recentProjectsList = document.getElementById("recent-projects-list");
  
  newIntakeDir = document.getElementById("new-intake-dir");
  newSelectedDir = document.getElementById("new-selected-dir");
  newDeletedDir = document.getElementById("new-deleted-dir");
  newProjectFile = document.getElementById("new-project-file");
  newCacheLimit = document.getElementById("new-cache-limit");
  cacheLimitVal = document.getElementById("cache-limit-val");
  
  // Viewer elements
  mediaImage = document.getElementById("media-image");
  mediaVideo = document.getElementById("media-video");
  mediaLiveVideo = document.getElementById("media-live-video");
  mediaBackdrop = document.getElementById("media-backdrop");
  badgeLive = document.getElementById("badge-live");
  badgeVideo = document.getElementById("badge-video");
  preloadProgress = document.getElementById("preload-progress");
  controlBar = document.getElementById("control-bar");
  
  mediaFilename = document.getElementById("media-filename");
  mediaIndex = document.getElementById("media-index");
  statsBubble = document.getElementById("stats-bubble");
  preloaderIndicator = document.getElementById("preloader-indicator");
  actionOverlay = document.getElementById("action-overlay");
  
  // Buttons
  btnPrev = document.getElementById("btn-prev");
  btnNext = document.getElementById("btn-next");
  btnDelete = document.getElementById("btn-delete");
  btnSelect = document.getElementById("btn-select");
  btnUndo = document.getElementById("btn-undo");
  btnRotate = document.getElementById("btn-rotate");
  btnSaveRotate = document.getElementById("btn-save-rotate");
  btnRename = document.getElementById("btn-rename");
  btnSettings = document.getElementById("btn-settings");
  
  // Rename Modal
  renameModal = document.getElementById("rename-modal");
  renameInput = document.getElementById("rename-input");
  renameError = document.getElementById("rename-error");
  btnRenameCancel = document.getElementById("btn-rename-cancel");
  btnRenameConfirm = document.getElementById("btn-rename-confirm");
  
  // Settings Modal
  settingsModal = document.getElementById("settings-modal");
  settingsSelectedDir = document.getElementById("settings-selected-dir");
  settingsDeletedDir = document.getElementById("settings-deleted-dir");
  settingsCacheLimit = document.getElementById("settings-cache-limit");
  settingsCacheLimitVal = document.getElementById("settings-cache-limit-val");
  btnSettingsCancel = document.getElementById("btn-settings-cancel");
  btnSettingsSave = document.getElementById("btn-settings-save");
  btnSettingsBrowseSelected = document.getElementById("btn-settings-browse-selected");
  btnSettingsBrowseDeleted = document.getElementById("btn-settings-browse-deleted");
  
  // Toast
  toast = document.getElementById("toast");
  toastText = document.getElementById("toast-text");
}

// Tab Switching
function setupTabSystem() {
  tabOpen.addEventListener("click", () => {
    tabOpen.classList.add("active");
    tabNew.classList.remove("active");
    contentOpen.classList.remove("hidden");
    contentNew.classList.add("hidden");
  });
  
  tabNew.addEventListener("click", () => {
    tabNew.classList.add("active");
    tabOpen.classList.remove("active");
    contentNew.classList.remove("hidden");
    contentOpen.classList.add("hidden");
  });
  
  newCacheLimit.addEventListener("input", (e) => {
    cacheLimitVal.textContent = `${e.target.value} MiB`;
  });
  
  settingsCacheLimit.addEventListener("input", (e) => {
    settingsCacheLimitVal.textContent = `${e.target.value} MiB`;
  });
}

// Onboarding Operations
function setupOnboardingActions() {
  // Select existing project file
  btnSelectProject.addEventListener("click", async () => {
    try {
      const selected = await selectDirectory({
        multiple: false,
        directory: false,
        filters: [{ name: "picselect Project", extensions: ["json"] }]
      });
      if (selected) {
        await openProject(selected);
      }
    } catch (e) {
      showToast(`Error opening project: ${e}`, true);
    }
  });

  // Browse buttons for project creation
  btnBrowseIntake.addEventListener("click", async () => {
    const selected = await selectDirectory({ directory: true, multiple: false });
    if (selected) {
      newIntakeDir.value = selected;
      // Propose project file location
      newProjectFile.value = `${selected}/picselect.json`;
      validateCreateButton();
    }
  });

  btnBrowseSelected.addEventListener("click", async () => {
    const selected = await selectDirectory({ directory: true, multiple: false });
    if (selected) {
      newSelectedDir.value = selected;
    }
  });

  btnBrowseDeleted.addEventListener("click", async () => {
    const selected = await selectDirectory({ directory: true, multiple: false });
    if (selected) {
      newDeletedDir.value = selected;
    }
  });

  btnBrowseProjectFile.addEventListener("click", async () => {
    const selected = await saveFileDialog({
      filters: [{ name: "picselect Project", extensions: ["json"] }],
      defaultPath: "picselect.json"
    });
    if (selected) {
      newProjectFile.value = selected;
      validateCreateButton();
    }
  });

  newIntakeDir.addEventListener("input", validateCreateButton);
  newProjectFile.addEventListener("input", validateCreateButton);

  function validateCreateButton() {
    btnCreateProject.disabled = !newIntakeDir.value || !newProjectFile.value;
  }

  btnCreateProject.addEventListener("click", async () => {
    try {
      const intake = newIntakeDir.value;
      const selected = newSelectedDir.value || "./selected";
      const deleted = newDeletedDir.value || "./deleted";
      const cacheLimit = parseInt(newCacheLimit.value);
      const projFile = newProjectFile.value;
      
      const config = await invoke("create_project", {
        intakeDir: intake,
        selectedDir: selected,
        deletedDir: deleted,
        cacheLimitMib: cacheLimit,
        projectFilePath: projFile
      });
      
      activeConfig = config;
      projectFilePath = projFile;
      
      showToast("Project created successfully!");
      switchScreen("main-screen");
      await discoverMedia();
    } catch (e) {
      showToast(`Failed to create project: ${e}`, true);
    }
  });
}

// Load Recent Projects
async function loadRecentProjects() {
  try {
    const recents = await invoke("get_recent_projects");
    recentProjectsList.innerHTML = "";
    
    if (!recents.all_recent_paths || recents.all_recent_paths.length === 0) {
      recentProjectsList.innerHTML = `<div class="no-recents">No recent projects found.</div>`;
      return;
    }
    
    recents.all_recent_paths.forEach(path => {
      const el = document.createElement("div");
      el.className = "recent-item";
      el.innerHTML = `
        <span class="recent-path" title="${path}">${path}</span>
        <span class="recent-arrow">→</span>
      `;
      el.addEventListener("click", () => openProject(path));
      recentProjectsList.appendChild(el);
    });
  } catch (e) {
    console.error("Failed to load recents:", e);
  }
}

// Open Project Handler
async function openProject(path) {
  try {
    preloaderIndicator.classList.add("active");
    const config = await invoke("open_project", { filePath: path });
    activeConfig = config;
    projectFilePath = path;
    showToast("Project loaded successfully!");
    switchScreen("main-screen");
    await discoverMedia();
  } catch (e) {
    showToast(`Error: ${e}`, true);
  } finally {
    preloaderIndicator.classList.remove("active");
  }
}

// Media Discovery & List Scanning
async function discoverMedia(targetMediaId = null) {
  try {
    preloaderIndicator.classList.add("active");
    mediaItems = await invoke("get_media");
    
    if (mediaItems.length === 0) {
      currentIndex = -1;
      renderEmptyState();
      return;
    }
    
    // If a target ID is specified (like after an undo), find and jump to it
    if (targetMediaId) {
      const idx = mediaItems.findIndex(item => item.id === targetMediaId);
      if (idx !== -1) {
        currentIndex = idx;
      } else {
        currentIndex = 0;
      }
    } else if (currentIndex < 0 || currentIndex >= mediaItems.length) {
      currentIndex = 0;
    }
    
    await renderMediaItem();
  } catch (e) {
    showToast(`Failed to discover media: ${e}`, true);
  } finally {
    preloaderIndicator.classList.remove("active");
  }
}

// Rendering Core Media
async function renderMediaItem() {
  if (currentIndex < 0 || currentIndex >= mediaItems.length) {
    renderEmptyState();
    return;
  }
  
  const item = mediaItems[currentIndex];
  
  // Reset states
  currentRotation = item.rotation;
  btnSaveRotate.classList.add("hidden");
  isLivePlayback = false;
  
  // Text content updates
  mediaFilename.textContent = item.name;
  mediaIndex.textContent = `${currentIndex + 1} / ${mediaItems.length}`;
  
  // Apply visual rotation from item (EXIF orientation tag translation)
  applyRotation(currentRotation);
  
  // Render based on media type
  badgeLive.classList.add("hidden");
  badgeVideo.classList.add("hidden");
  mediaImage.classList.remove("active");
  mediaVideo.classList.remove("active");
  mediaLiveVideo.classList.remove("active");
  mediaVideo.pause();
  mediaLiveVideo.pause();
  
  // Enforce background preloader lookahead prefetching (sends active window update first!)
  triggerLookahead();

  // Setup URLs via custom tauri scheme `picselect-asset` and update backdrop image
  if (item.is_live) {
    badgeLive.classList.remove("hidden");
    const backdropUrl = `picselect-asset://localhost${item.image_path}`;
    mediaBackdrop.style.backgroundImage = `url("${backdropUrl}")`;
    mediaBackdrop.classList.add("active");
    
    mediaImage.src = backdropUrl;
    mediaImage.classList.add("active");
    mediaLiveVideo.src = `picselect-asset://localhost${item.video_path}`;
  } else if (item.has_video) {
    badgeVideo.classList.remove("hidden");
    mediaVideo.src = `picselect-asset://localhost${item.video_path}`;
    mediaVideo.classList.add("active");
    mediaVideo.load();
    
    // Clear backdrop for standalone videos
    mediaBackdrop.style.backgroundImage = "";
    mediaBackdrop.classList.remove("active");
  } else if (item.has_image) {
    const backdropUrl = `picselect-asset://localhost${item.image_path}`;
    mediaBackdrop.style.backgroundImage = `url("${backdropUrl}")`;
    mediaBackdrop.classList.add("active");
    
    mediaImage.src = backdropUrl;
    mediaImage.classList.add("active");
  }

  updateMediaScaling();
}

function renderEmptyState() {
  mediaImage.src = "";
  mediaImage.classList.remove("active");
  mediaVideo.src = "";
  mediaVideo.classList.remove("active");
  mediaLiveVideo.src = "";
  mediaLiveVideo.classList.remove("active");
  
  badgeLive.classList.add("hidden");
  badgeVideo.classList.add("hidden");
  
  mediaFilename.textContent = "No Pictures Found";
  mediaIndex.textContent = "0 / 0";
  preloadProgress.style.width = "0%";
  
  mediaBackdrop.style.backgroundImage = "";
  mediaBackdrop.classList.remove("active");
}

// Caching Lookahead Prefetch coordinator
async function triggerLookahead() {
  if (mediaItems.length === 0) return;
  
  // Determine next 5 media items to pre-buffer
  const lookaheadWindowSize = 5;
  const lookaheadItems = [];
  const lookaheadIds = [];
  
  for (let i = 0; i <= lookaheadWindowSize; i++) {
    const nextIdx = (currentIndex + i) % mediaItems.length;
    // Don't wrap around to active index
    if (i > 0 && nextIdx === currentIndex) break;
    
    const item = mediaItems[nextIdx];
    lookaheadItems.push(item);
    lookaheadIds.push(item.id);
  }
  
  try {
    const stats = await invoke("set_active_lookahead", {
      activeMediaItems: lookaheadItems,
      lookaheadMediaIds: lookaheadIds
    });
    
    // Update preloader progress bar UI
    const fillPercent = (stats.l2_count / Math.min(mediaItems.length, lookaheadWindowSize + 1)) * 100;
    preloadProgress.style.width = `${Math.min(100, fillPercent)}%`;
    
    // Update Stats
    updateStatsBubble(stats);
  } catch (e) {
    console.error("Lookahead failed:", e);
  }
}

// Main actions (Keep, Discard, Navigation, Undos)
function setupMainViewerActions() {
  btnPrev.addEventListener("click", navigatePrev);
  btnNext.addEventListener("click", navigateNext);
  btnSelect.addEventListener("click", keepItem);
  btnDelete.addEventListener("click", discardItem);
  btnUndo.addEventListener("click", undoAction);
  
  btnRotate.addEventListener("click", () => rotateItem(90));
  btnSaveRotate.addEventListener("click", saveRotation);
  btnRename.addEventListener("click", openRenameModal);

  // Listen to loads to dynamically update scaling
  mediaImage.addEventListener("load", updateMediaScaling);
  mediaVideo.addEventListener("loadedmetadata", updateMediaScaling);
  mediaLiveVideo.addEventListener("loadedmetadata", updateMediaScaling);

  // Resize and Fullscreen change listeners
  window.addEventListener("resize", updateMediaScaling);
  document.addEventListener("fullscreenchange", updateMediaScaling);
}

function navigateNext() {
  if (mediaItems.length <= 1) return;
  currentIndex = (currentIndex + 1) % mediaItems.length;
  renderMediaItem();
  triggerControlBarReveal();
}

function navigatePrev() {
  if (mediaItems.length <= 1) return;
  currentIndex = (currentIndex - 1 + mediaItems.length) % mediaItems.length;
  renderMediaItem();
  triggerControlBarReveal();
}

async function keepItem() {
  if (currentIndex < 0 || mediaItems.length === 0) return;
  
  const item = mediaItems[currentIndex];
  showActionOverlay("keep", "Keep");
  
  try {
    await invoke("move_media_item", { mediaItem: item, toSelected: true });
    
    // Remove from active list
    mediaItems.splice(currentIndex, 1);
    if (mediaItems.length === 0) {
      currentIndex = -1;
    } else if (currentIndex >= mediaItems.length) {
      currentIndex = 0;
    }
    
    await renderMediaItem();
  } catch (e) {
    showToast(`Error keeping item: ${e}`, true);
  }
}

async function discardItem() {
  if (currentIndex < 0 || mediaItems.length === 0) return;
  
  const item = mediaItems[currentIndex];
  showActionOverlay("delete", "Discard");
  
  try {
    await invoke("move_media_item", { mediaItem: item, toSelected: false });
    
    // Remove from active list
    mediaItems.splice(currentIndex, 1);
    if (mediaItems.length === 0) {
      currentIndex = -1;
    } else if (currentIndex >= mediaItems.length) {
      currentIndex = 0;
    }
    
    await renderMediaItem();
  } catch (e) {
    showToast(`Error discarding item: ${e}`, true);
  }
}

async function undoAction() {
  try {
    const undoneMediaId = await invoke("undo_action");
    if (undoneMediaId) {
      showActionOverlay("undo", "Undo");
      showToast("Last action undone!");
      await discoverMedia(undoneMediaId);
    } else {
      showToast("Nothing to undo.");
    }
  } catch (e) {
    showToast(`Undo failed: ${e}`, true);
  }
}

// Visual Rotations
function rotateItem(deg) {
  currentRotation = (currentRotation + deg) % 360;
  applyRotation(currentRotation);
  btnSaveRotate.classList.remove("hidden");
}

function applyRotation(deg) {
  mediaImage.style.transform = `rotate(${deg}deg)`;
  mediaVideo.style.transform = `rotate(${deg}deg)`;
  mediaLiveVideo.style.transform = `rotate(${deg}deg)`;
  updateMediaScaling();
}

async function saveRotation() {
  if (currentIndex < 0) return;
  const item = mediaItems[currentIndex];
  
  try {
    preloaderIndicator.classList.add("active");
    await invoke("write_rotation_to_file", { mediaItem: item, rotation: currentRotation });
    showToast("Rotation saved to file!");
    btnSaveRotate.classList.add("hidden");
    
    // Reset local rotation to 0 because the physical EXIF file is now modified,
    // and the browser natively auto-rotates the updated file upright!
    mediaItems[currentIndex].rotation = 0;
    currentRotation = 0;
    applyRotation(0);
  } catch (e) {
    showToast(`Failed to save rotation: ${e}`, true);
  } finally {
    preloaderIndicator.classList.remove("active");
  }
}

// Rename Dialogs
function setupRenameActions() {
  btnRenameCancel.addEventListener("click", closeRenameModal);
  btnRenameConfirm.addEventListener("click", confirmRename);
  
  renameInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      confirmRename();
    } else if (e.key === "Escape") {
      closeRenameModal();
    }
  });
}

function openRenameModal() {
  if (currentIndex < 0) return;
  const item = mediaItems[currentIndex];
  renameInput.value = item.name;
  renameError.textContent = "";
  renameModal.classList.remove("hidden");
  renameInput.focus();
  renameInput.select();
}

function closeRenameModal() {
  renameModal.classList.add("hidden");
}

async function confirmRename() {
  const newName = renameInput.value.trim();
  if (!newName) {
    renameError.textContent = "Name cannot be empty.";
    return;
  }
  
  const item = mediaItems[currentIndex];
  if (newName === item.name) {
    closeRenameModal();
    return;
  }
  
  try {
    preloaderIndicator.classList.add("active");
    await invoke("rename_media_item", { mediaItem: item, newName });
    showToast("File renamed successfully!");
    closeRenameModal();
    
    // Rescan directory to reflect new path strings
    await discoverMedia(newName);
  } catch (e) {
    renameError.textContent = e;
  } finally {
    preloaderIndicator.classList.remove("active");
  }
}

// Settings Modal Action
function setupSettingsActions() {
  btnSettings.addEventListener("click", openSettingsModal);
  btnSettingsCancel.addEventListener("click", closeSettingsModal);
  btnSettingsSave.addEventListener("click", saveSettings);
  
  btnSettingsBrowseSelected.addEventListener("click", async () => {
    const selected = await selectDirectory({ directory: true, multiple: false });
    if (selected) {
      settingsSelectedDir.value = selected;
    }
  });

  btnSettingsBrowseDeleted.addEventListener("click", async () => {
    const selected = await selectDirectory({ directory: true, multiple: false });
    if (selected) {
      settingsDeletedDir.value = selected;
    }
  });
  
  window.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && !settingsModal.classList.contains("hidden")) {
      closeSettingsModal();
    }
  });
}

function openSettingsModal() {
  if (!activeConfig) return;
  
  settingsSelectedDir.value = activeConfig.selected_dir;
  settingsDeletedDir.value = activeConfig.deleted_dir;
  settingsCacheLimit.value = activeConfig.cache_limit_mib;
  settingsCacheLimitVal.textContent = `${activeConfig.cache_limit_mib} MiB`;
  
  settingsModal.classList.remove("hidden");
}

function closeSettingsModal() {
  settingsModal.classList.add("hidden");
}

async function saveSettings() {
  try {
    const selected = settingsSelectedDir.value;
    const deleted = settingsDeletedDir.value;
    const cacheLimit = parseInt(settingsCacheLimit.value);
    
    const config = await invoke("save_project_settings", {
      selectedDir: selected,
      deletedDir: deleted,
      cacheLimitMib: cacheLimit
    });
    
    activeConfig = config;
    showToast("Settings saved!");
    closeSettingsModal();
  } catch (e) {
    showToast(`Failed to save settings: ${e}`, true);
  }
}

// Keyboard shortcuts (J/K, Arrow Keys, Space, D, Enter, CMD+Z, R, W, T, Comma)
function setupKeyboardShortcuts() {
  // Bind auxiliary setup calls
  setupRenameActions();
  setupSettingsActions();

  window.addEventListener("keydown", (e) => {
    // Ignore key binds if inputs or modals are open
    if (document.activeElement.tagName === "INPUT" || 
        !renameModal.classList.contains("hidden") ||
        !settingsModal.classList.contains("hidden")) {
      return;
    }
    
    const key = e.key.toLowerCase();
    
    // 1. Navigation shortcuts
    if (key === "j" || e.key === "ArrowDown" || e.key === "ArrowRight") {
      e.preventDefault();
      navigateNext();
    } else if (key === "k" || e.key === "ArrowUp" || e.key === "ArrowLeft") {
      e.preventDefault();
      navigatePrev();
    }
    
    // 2. Operations shortcuts
    else if (key === "d" || e.key === "Delete" || e.key === "Backspace") {
      e.preventDefault();
      discardItem();
    } else if (e.key === "Enter") {
      e.preventDefault();
      keepItem();
    } else if (key === "u" || (e.metaKey && key === "z")) {
      e.preventDefault();
      undoAction();
    }
    
    // 3. Rotations & Rename
    else if (key === "r") {
      e.preventDefault();
      if (e.shiftKey) {
        rotateItem(-90); // counter-clockwise
      } else {
        rotateItem(90);  // clockwise
      }
    } else if (key === "w") {
      e.preventDefault();
      if (!btnSaveRotate.classList.contains("hidden")) {
        saveRotation();
      }
    } else if (key === "t") {
      e.preventDefault();
      openRenameModal();
    }
    
    // 4. Modal Triggers
    else if (e.key === ",") {
      e.preventDefault();
      openSettingsModal();
    }
    
    // 5. Video Playbacks / Live Photos
    else if (e.key === " ") {
      e.preventDefault();
      handlePlaybackToggle();
    }
  });

  window.addEventListener("keyup", (e) => {
    if (e.key === " ") {
      e.preventDefault();
      stopLivePhotoPlayback();
    }
  });
}

// Live Photo & Video playback coordinator
function handlePlaybackToggle() {
  if (currentIndex < 0) return;
  const item = mediaItems[currentIndex];
  
  if (item.is_live) {
    startLivePhotoPlayback();
  } else if (item.has_video) {
    if (mediaVideo.paused) {
      mediaVideo.play();
      showToast("Playing video");
    } else {
      mediaVideo.pause();
      showToast("Paused video");
    }
  }
}

function startLivePhotoPlayback() {
  if (isLivePlayback) return;
  isLivePlayback = true;
  
  mediaLiveVideo.classList.add("active");
  mediaImage.classList.remove("active");
  
  mediaLiveVideo.currentTime = 0;
  mediaLiveVideo.loop = true;
  mediaLiveVideo.play();
  
  showToast("Playing Live Photo");
}

function stopLivePhotoPlayback() {
  if (!isLivePlayback) return;
  isLivePlayback = false;
  
  mediaLiveVideo.pause();
  mediaLiveVideo.classList.remove("active");
  mediaImage.classList.add("active");
  
  showToast("Stopped Live Photo");
}

// Auto-hide controls bar when mouse is idle
function setupAutoHideControls() {
  window.addEventListener("mousemove", triggerControlBarReveal);
  window.addEventListener("keydown", triggerControlBarReveal);
  triggerControlBarReveal();
}

function triggerControlBarReveal() {
  controlBar.classList.remove("hide");
  document.body.style.cursor = "default";
  
  clearTimeout(autoHideTimeout);
  autoHideTimeout = setTimeout(() => {
    // Only auto-hide if settings/rename modals are not open
    if (renameModal.classList.contains("hidden") && 
        settingsModal.classList.contains("hidden") &&
        currentIndex >= 0) {
      controlBar.classList.add("hide");
      document.body.style.cursor = "none";
    }
  }, 2500);
}

// Visual Helpers
function switchScreen(screenId) {
  onboardingScreen.classList.remove("active");
  mainScreen.classList.remove("active");
  
  document.getElementById(screenId).classList.add("active");
}

function showToast(text, isError = false) {
  toastText.textContent = text;
  toast.classList.remove("hidden");
  
  if (isError) {
    toast.style.borderColor = "var(--color-discard)";
    toast.style.color = "var(--color-discard)";
  } else {
    toast.style.borderColor = "var(--glass-border)";
    toast.style.color = "var(--text-primary)";
  }
  
  toast.classList.add("active");
  
  setTimeout(() => {
    toast.classList.remove("active");
  }, 3000);
}

function showActionOverlay(actionType, text) {
  actionOverlay.className = `action-overlay active ${actionType}`;
  
  const iconSpan = actionOverlay.querySelector(".action-icon");
  const textSpan = actionOverlay.querySelector(".action-text");
  
  textSpan.textContent = text;
  
  if (actionType === "keep") {
    iconSpan.innerHTML = `<svg viewBox="0 0 24 24" width="36" height="36" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="20 6 9 17 4 12"></polyline></svg>`;
  } else if (actionType === "delete") {
    iconSpan.innerHTML = `<svg viewBox="0 0 24 24" width="36" height="36" fill="none" stroke="currentColor" stroke-width="3"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>`;
  } else if (actionType === "undo") {
    iconSpan.innerHTML = `<svg viewBox="0 0 24 24" width="32" height="32" fill="none" stroke="currentColor" stroke-width="2.5"><path d="M3 7v6h6"></path><path d="M21 17a9 9 0 0 0-9-9 9 9 0 0 0-6 2.3L3 13"></path></svg>`;
  }
  
  setTimeout(() => {
    actionOverlay.classList.remove("active");
  }, 600);
}

// Statistics Bubble & RAM updates
async function updateCacheStats() {
  if (currentIndex < 0) return;
  try {
    const stats = await invoke("get_cache_stats");
    updateStatsBubble(stats);
  } catch (e) {
    console.error(e);
  }
}

function updateStatsBubble(stats) {
  statsBubble.textContent = `Cache: ${stats.current_memory_mib.toFixed(1)} MiB / ${stats.max_memory_mib.toFixed(0)} MiB (${stats.l1_count} L1, ${stats.l2_count} L2)`;
}

// Dynamically adjusts media object-fit scaling based on windowed vs fullscreen dimensions.
// By default, using 'contain' ensures media is perfectly centered and never cut off,
// while the blurred ambient backdrop absorbs any empty aspect-ratio border spaces.
function updateMediaScaling() {
  mediaImage.style.objectFit = "contain";
  mediaVideo.style.objectFit = "contain";
  mediaLiveVideo.style.objectFit = "contain";
}
