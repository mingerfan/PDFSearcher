<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  const { filePath, initialPage = 1, isVisible, onClose } = $props<{ 
    filePath: string; 
    initialPage?: number; 
    isVisible: boolean;
    onClose: () => void;
  }>();
  let pdfData = $state<string>("");
  let currentPage = $state(initialPage);
  let totalPages = $state(1);
  let loading = $state(false);
  let error = $state("");
  let pdfCanvas = $state<HTMLCanvasElement>();
  let pdfDoc: any = null;
  let scale = $state(1.2);
  let showThumbnails = $state(true);
  let thumbnails = $state<Array<{ pageNum: number; canvas: HTMLCanvasElement; loaded: boolean }>>([]);
  let searchText = $state("");
  let searchResults = $state<Array<{ pageNum: number; matches: number }>>([]);
  let isSearching = $state(false);
  let fitMode = $state<'width' | 'height' | 'page'>('width');
  let rotateAngle = $state(0);
  let bookmarks = $state<Array<{ pageNum: number; label: string }>>([]);
  let showBookmarks = $state(false);
  let isInitializing = $state(false);

  // 等待Canvas元素准备就绪
  async function waitForCanvas(maxAttempts = 10): Promise<boolean> {
    for (let i = 0; i < maxAttempts; i++) {
      if (pdfCanvas) {
        return true;
      }
      await new Promise(resolve => setTimeout(resolve, 100));
    }
    return false;
  }
  
  // 监听isVisible变化
  $effect(() => {
    if (isVisible && !pdfDoc && !isInitializing) {
      initializePDF();
    }
  });
  async function initializePDF() {
    if (!isVisible || isInitializing) return;
    
    isInitializing = true;
    loading = true;
    error = "";
    
    try {
      const pdfjsLib = await import("pdfjs-dist");
      pdfjsLib.GlobalWorkerOptions.workerSrc = "/js/pdf.worker.min.js";

      const base64Data = await invoke<string>("get_pdf_base64", {
        filePath: filePath,
      });

      const binaryString = atob(base64Data);
      const bytes = new Uint8Array(binaryString.length);
      for (let i = 0; i < binaryString.length; i++) {
        bytes[i] = binaryString.charCodeAt(i);
      }

      const loadingTask = pdfjsLib.getDocument({ data: bytes });
      pdfDoc = await loadingTask.promise;
      totalPages = pdfDoc.numPages;
      
      loading = false;
      
      // 等待DOM渲染完成再等待Canvas
      await new Promise(resolve => setTimeout(resolve, 50));
      
      const canvasReady = await waitForCanvas();
      if (!canvasReady) {
        error = "Canvas元素初始化失败";
        isInitializing = false;
        return;
      }
      
      await renderPage(currentPage);
      await generateThumbnails();
    } catch (e) {
      console.error("Failed to load PDF:", e);
      error = `无法加载PDF文件: ${e}`;
      loading = false;
    } finally {
      isInitializing = false;
    }
  }

  async function renderPage(pageNum: number) {
    if (!pdfDoc || !pdfCanvas) return;

    try {
      const page = await pdfDoc.getPage(pageNum);
      let viewport = page.getViewport({ scale, rotation: rotateAngle });
      
      // 根据适配模式调整scale
      if (fitMode === 'width') {
        const containerWidth = pdfCanvas.parentElement?.clientWidth || 800;
        scale = (containerWidth - 40) / viewport.width;
        viewport = page.getViewport({ scale, rotation: rotateAngle });
      } else if (fitMode === 'height') {
        const containerHeight = pdfCanvas.parentElement?.clientHeight || 600;
        scale = (containerHeight - 40) / viewport.height;
        viewport = page.getViewport({ scale, rotation: rotateAngle });
      }
      
      pdfCanvas.height = viewport.height;
      pdfCanvas.width = viewport.width;

      const context = pdfCanvas.getContext("2d");
      if (!context) return;
      
      context.clearRect(0, 0, pdfCanvas.width, pdfCanvas.height);
      context.fillStyle = "#ffffff";
      context.fillRect(0, 0, pdfCanvas.width, pdfCanvas.height);
      
      await page.render({
        canvasContext: context,
        viewport: viewport,
      }).promise;
      
      error = "";
    } catch (e) {
      console.error("Failed to render page:", e);
      error = `无法渲染PDF页面 ${pageNum}: ${e}`;
    }
  }  async function generateThumbnails() {
    if (!pdfDoc) return;
    
    console.log("开始生成缩略图...", totalPages, "页");
    thumbnails = [];
    const maxThumbnails = Math.min(totalPages, 100); // 限制缩略图数量以提高性能
    
    for (let i = 1; i <= maxThumbnails; i++) {
      thumbnails.push({ pageNum: i, canvas: document.createElement('canvas'), loaded: false });
    }
    
    // 触发UI更新
    thumbnails = [...thumbnails];
    
    // 批量生成缩略图，避免阻塞UI
    let batchSize = 3; // 减少批次大小
    for (let i = 0; i < thumbnails.length; i += batchSize) {
      const batch = thumbnails.slice(i, i + batchSize);
      await Promise.all(batch.map((_, index) => generateThumbnail(i + index)));
      
      // 每批次之间添加短暂延迟，避免阻塞主线程
      await new Promise(resolve => setTimeout(resolve, 50));
    }
    
    console.log("缩略图生成完成");
  }

  async function generateThumbnail(index: number) {
    if (!pdfDoc || !thumbnails[index]) return;
    
    try {
      const page = await pdfDoc.getPage(thumbnails[index].pageNum);
      const viewport = page.getViewport({ scale: 0.2 });
      
      const canvas = thumbnails[index].canvas;
      canvas.height = viewport.height;
      canvas.width = viewport.width;
      
      const context = canvas.getContext("2d");
      if (!context) return;
      
      context.fillStyle = "#ffffff";
      context.fillRect(0, 0, canvas.width, canvas.height);
      
      await page.render({
        canvasContext: context,
        viewport: viewport,
      }).promise;
      
      thumbnails[index].loaded = true;
      thumbnails = [...thumbnails]; // 触发响应式更新
    } catch (e) {
      console.error("Failed to generate thumbnail:", e);
    }
  }

  async function goToPage(pageNum: number) {
    if (pageNum >= 1 && pageNum <= totalPages && pageNum !== currentPage) {
      currentPage = pageNum;
      await renderPage(currentPage);
    }
  }

  async function nextPage() {
    await goToPage(currentPage + 1);
  }

  async function prevPage() {
    await goToPage(currentPage - 1);
  }

  async function zoomIn() {
    if (fitMode !== 'page') {
      scale = Math.min(scale * 1.2, 5.0);
      await renderPage(currentPage);
    }
  }

  async function zoomOut() {
    if (fitMode !== 'page') {
      scale = Math.max(scale / 1.2, 0.2);
      await renderPage(currentPage);
    }
  }

  async function setFitMode(mode: 'width' | 'height' | 'page') {
    fitMode = mode;
    await renderPage(currentPage);
  }

  async function rotate() {
    rotateAngle = (rotateAngle + 90) % 360;
    await renderPage(currentPage);
  }
  function toggleThumbnails() {
    showThumbnails = !showThumbnails;
    showBookmarks = false; // 关闭书签面板
  }

  function toggleBookmarks() {
    showBookmarks = !showBookmarks;
    showThumbnails = false; // 关闭缩略图面板
  }

  function addBookmark() {
    const label = prompt(`为第 ${currentPage} 页添加书签:`, `页面 ${currentPage}`);
    if (label) {
      bookmarks = [...bookmarks, { pageNum: currentPage, label }];
    }
  }

  function removeBookmark(pageNum: number) {
    bookmarks = bookmarks.filter(b => b.pageNum !== pageNum);
  }

  async function searchInPDF() {
    if (!searchText.trim() || !pdfDoc) return;
    
    isSearching = true;
    searchResults = [];
    
    try {
      for (let i = 1; i <= totalPages; i++) {
        const page = await pdfDoc.getPage(i);
        const textContent = await page.getTextContent();
        const text = textContent.items.map((item: any) => item.str).join(' ');
        const matches = (text.toLowerCase().match(new RegExp(searchText.toLowerCase(), 'g')) || []).length;
        
        if (matches > 0) {
          searchResults.push({ pageNum: i, matches });
        }
      }
    } catch (e) {
      console.error("Search failed:", e);
    }
    
    isSearching = false;
  }  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      onClose();
    } else if (event.key === "ArrowLeft" || event.key === "PageUp") {
      prevPage();
    } else if (event.key === "ArrowRight" || event.key === "PageDown") {
      nextPage();
    } else if (event.key === "+" || event.key === "=") {
      zoomIn();
    } else if (event.key === "-") {
      zoomOut();
    } else if (event.key === "Home") {
      goToPage(1);
    } else if (event.key === "End") {
      goToPage(totalPages);
    }
  }

  function handleWheel(event: WheelEvent) {
    if (event.ctrlKey) {
      event.preventDefault();
      if (event.deltaY < 0) {
        zoomIn();
      } else {
        zoomOut();
      }
    }
  }

  // 自定义指令：渲染缩略图到canvas
  function renderThumbnailCanvas(canvas: HTMLCanvasElement, sourceCanvas: HTMLCanvasElement) {
    const ctx = canvas.getContext('2d');
    if (ctx && sourceCanvas) {
      ctx.drawImage(sourceCanvas, 0, 0);
    }
    
    return {
      update(newSourceCanvas: HTMLCanvasElement) {
        const ctx = canvas.getContext('2d');
        if (ctx && newSourceCanvas) {
          ctx.drawImage(newSourceCanvas, 0, 0);
        }
      }
    };
  }
</script>

<svelte:window on:keydown={handleKeydown} />

<div class="modern-pdf-viewer" class:visible={isVisible}>  <!-- 侧边栏 -->
  <div class="sidebar" class:collapsed={!showThumbnails && !showBookmarks}>
    <div class="sidebar-header">      <button class="icon-btn" onclick={toggleThumbnails} class:active={showThumbnails} title="缩略图" aria-label="切换缩略图显示">
        <svg width="16" height="16" viewBox="0 0 16 16">
          <path d="M2 2h4v4H2V2zm6 0h4v4H8V2zM2 8h4v4H2V8zm6 0h4v4H8V8z" fill="currentColor"/>
        </svg>
      </button>
      <button class="icon-btn" onclick={toggleBookmarks} class:active={showBookmarks} title="书签" aria-label="切换书签显示">
        <svg width="16" height="16" viewBox="0 0 16 16">
          <path d="M2 2a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v13.5a.5.5 0 0 1-.777.416L8 13.101l-5.223 2.815A.5.5 0 0 1 2 15.5V2zm2-1a1 1 0 0 0-1 1v12.566l4.723-2.482a.5.5 0 0 1 .554 0L13 14.566V2a1 1 0 0 0-1-1H4z" fill="currentColor"/>
        </svg>
      </button>
      <span class="sidebar-title">
        {showThumbnails ? '页面缩略图' : showBookmarks ? '书签' : '导航'}
      </span>
    </div>
    
    {#if showThumbnails}
      <div class="search-section">
        <div class="search-box">
          <input 
            type="text" 
            bind:value={searchText} 
            placeholder="搜索文本..."
            onkeydown={(e) => e.key === 'Enter' && searchInPDF()}
          />
          <button onclick={searchInPDF} disabled={isSearching} class="search-btn">
            {#if isSearching}
              <div class="spinner"></div>
            {:else}
              🔍
            {/if}
          </button>
        </div>
        
        {#if searchResults.length > 0}
          <div class="search-results">
            <div class="search-results-header">找到 {searchResults.length} 个匹配页面</div>
            {#each searchResults as result}
              <button 
                class="search-result-item"
                onclick={() => goToPage(result.pageNum)}
              >
                第 {result.pageNum} 页 ({result.matches} 个匹配)
              </button>
            {/each}
          </div>
        {/if}
      </div>
      
      <div class="thumbnails-container">        {#each thumbnails as thumbnail}
          <div 
            class="thumbnail"
            class:active={thumbnail.pageNum === currentPage}
            onclick={() => goToPage(thumbnail.pageNum)}
          >
            {#if thumbnail.loaded}
              <div class="thumbnail-canvas-container">
                <canvas
                  data-page={thumbnail.pageNum}
                  width={thumbnail.canvas.width}
                  height={thumbnail.canvas.height}
                  style="width: 100%; height: auto;"
                  use:renderThumbnailCanvas={thumbnail.canvas}
                ></canvas>
              </div>
            {:else}
              <div class="thumbnail-loading">
                <div class="spinner"></div>
              </div>
            {/if}
            <div class="thumbnail-label">第 {thumbnail.pageNum} 页</div>
          </div>        {/each}
      </div>
    {/if}

    {#if showBookmarks}
      <div class="bookmarks-section">
        <div class="bookmarks-header">          <button onclick={addBookmark} class="add-bookmark-btn" title="添加书签" aria-label="为当前页面添加书签">
            <svg width="14" height="14" viewBox="0 0 16 16">
              <path d="M8 1a7 7 0 1 0 0 14A7 7 0 0 0 8 1zM4 8a.5.5 0 0 1 .5-.5h3v-3a.5.5 0 0 1 1 0v3h3a.5.5 0 0 1 0 1h-3v3a.5.5 0 0 1-1 0v-3h-3A.5.5 0 0 1 4 8z" fill="currentColor"/>
            </svg>
            添加书签
          </button>
        </div>
        
        <div class="bookmarks-list">
          {#each bookmarks as bookmark}
            <div class="bookmark-item">
              <button 
                class="bookmark-link"
                onclick={() => goToPage(bookmark.pageNum)}
                title="跳转到第 {bookmark.pageNum} 页"
              >
                <div class="bookmark-label">{bookmark.label}</div>
                <div class="bookmark-page">第 {bookmark.pageNum} 页</div>
              </button>              <button 
                class="remove-bookmark-btn"
                onclick={() => removeBookmark(bookmark.pageNum)}
                title="删除书签"
                aria-label="删除第{bookmark.pageNum}页的书签"
              >
                <svg width="12" height="12" viewBox="0 0 16 16">
                  <path d="M12.854 4.854l-8 8-.708-.708 8-8 .708.708z" fill="currentColor"/>
                  <path d="M4.146 4.854l8 8-.708.708-8-8 .708-.708z" fill="currentColor"/>
                </svg>
              </button>
            </div>
          {/each}
          
          {#if bookmarks.length === 0}
            <div class="no-bookmarks">
              暂无书签<br>
              <small>点击"添加书签"为当前页面添加书签</small>
            </div>
          {/if}
        </div>
      </div>
    {/if}
  </div>

  <!-- 主内容区 -->
  <div class="main-content">
    <!-- 工具栏 -->
    <div class="toolbar">
      <div class="toolbar-section">        <button onclick={onClose} class="close-btn" title="关闭" aria-label="关闭PDF查看器">
          <svg width="16" height="16" viewBox="0 0 16 16">
            <path d="M12.854 4.854l-8 8-.708-.708 8-8 .708.708z" fill="currentColor"/>
            <path d="M4.146 4.854l8 8-.708.708-8-8 .708-.708z" fill="currentColor"/>
          </svg>
        </button>
        <span class="file-name">{filePath.split("\\").pop()}</span>
      </div>
      
      <div class="toolbar-section">        <button onclick={prevPage} disabled={currentPage <= 1} title="上一页" aria-label="转到上一页">
          <svg width="16" height="16" viewBox="0 0 16 16">
            <path d="M10 2L4 8l6 6v-4h6V6h-6V2z" fill="currentColor"/>
          </svg>
        </button>
          <div class="page-input">
          <input 
            type="number" 
            bind:value={currentPage} 
            min="1" 
            max={totalPages}
            onkeydown={(e) => {
              if (e.key === 'Enter') {
                goToPage(currentPage);
              }
            }}
            onblur={() => goToPage(currentPage)}
          />
          <span>/ {totalPages}</span>
        </div>
          <button onclick={nextPage} disabled={currentPage >= totalPages} title="下一页" aria-label="转到下一页">
          <svg width="16" height="16" viewBox="0 0 16 16">
            <path d="M6 2v4H0v4h6v4l6-6-6-6z" fill="currentColor"/>
          </svg>
        </button>
      </div>

      <div class="toolbar-section">        <button onclick={zoomOut} disabled={fitMode !== 'page'} title="缩小" aria-label="缩小视图">
          <svg width="16" height="16" viewBox="0 0 16 16">
            <path d="M8 1C4.134 1 1 4.134 1 8s3.134 7 7 7 7-3.134 7-7-3.134-7-7-7zM5 7h6v2H5V7z" fill="currentColor"/>
          </svg>
        </button>
        
        <span class="zoom-display">{Math.round(scale * 100)}%</span>
        
        <button onclick={zoomIn} disabled={fitMode !== 'page'} title="放大" aria-label="放大视图">
          <svg width="16" height="16" viewBox="0 0 16 16">
            <path d="M8 1C4.134 1 1 4.134 1 8s3.134 7 7 7 7-3.134 7-7-3.134-7-7-7zM5 7h2V5h2v2h2v2H9v2H7V9H5V7z" fill="currentColor"/>
          </svg>
        </button>
        
        <div class="fit-buttons">
          <button 
            onclick={() => setFitMode('width')} 
            class:active={fitMode === 'width'}
            title="适合宽度"
          >
            宽度
          </button>
          <button 
            onclick={() => setFitMode('height')} 
            class:active={fitMode === 'height'}
            title="适合高度"
          >
            高度
          </button>
          <button 
            onclick={() => setFitMode('page')} 
            class:active={fitMode === 'page'}
            title="适合页面"
          >
            页面
          </button>
        </div>          <button onclick={rotate} title="旋转" aria-label="顺时针旋转90度">
          <svg width="16" height="16" viewBox="0 0 16 16">
            <path d="M8 1C4.134 1 1 4.134 1 8s3.134 7 7 7c1.934 0 3.659-.788 4.915-2.058l-1.414-1.414C10.537 12.392 9.32 13 8 13c-2.761 0-5-2.239-5-5s2.239-5 5-5 5 2.239 5 5h-2l3 3 3-3h-2c0-3.866-3.134-7-7-7z" fill="currentColor"/>
          </svg>
        </button>

        <button onclick={addBookmark} title="添加书签" aria-label="为当前页面添加书签">
          <svg width="16" height="16" viewBox="0 0 16 16">
            <path d="M2 2a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v13.5a.5.5 0 0 1-.777.416L8 13.101l-5.223 2.815A.5.5 0 0 1 2 15.5V2zm2-1a1 1 0 0 0-1 1v12.566l4.723-2.482a.5.5 0 0 1 .554 0L13 14.566V2a1 1 0 0 0-1-1H4z" fill="currentColor"/>
          </svg>
        </button>
      </div>
    </div>    <!-- PDF内容 -->
    <div class="pdf-content" onwheel={handleWheel}>
      {#if loading}
        <div class="loading-state">
          <div class="spinner large"></div>
          <div>正在加载PDF文档...</div>
          <div class="loading-progress">请稍候</div>
        </div>
      {:else if error}
        <div class="error-state">
          <div class="error-icon">⚠️</div>
          <div class="error-message">{error}</div>
          <button onclick={() => initializePDF()} class="retry-btn">重试</button>
        </div>
      {:else}
        <div class="pdf-canvas-container">
          <canvas bind:this={pdfCanvas} class="pdf-canvas"></canvas>
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  .modern-pdf-viewer {
    position: fixed;
    top: 0;
    right: -60%;
    width: 60%;
    height: 100vh;
    background: #ffffff;
    box-shadow: -4px 0 20px rgba(0, 0, 0, 0.15);
    transition: right 0.3s cubic-bezier(0.4, 0, 0.2, 1);
    z-index: 1000;
    display: flex;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  }

  .modern-pdf-viewer.visible {
    right: 0;
  }

  .sidebar {
    width: 280px;
    background: #f8f9fa;
    border-right: 1px solid #e9ecef;
    display: flex;
    flex-direction: column;
    transition: width 0.2s ease;
  }
  .sidebar.collapsed {
    width: 48px;
  }

  .sidebar.collapsed .sidebar-title,
  .sidebar.collapsed .search-section,
  .sidebar.collapsed .thumbnails-container,
  .sidebar.collapsed .bookmarks-section {
    display: none;
  }

  .sidebar-header {
    padding: 12px 16px;
    border-bottom: 1px solid #e9ecef;
    display: flex;
    align-items: center;
    gap: 8px;
    background: #ffffff;
  }

  .sidebar-title {
    font-weight: 500;
    color: #495057;
    font-size: 14px;
  }

  .sidebar.collapsed .sidebar-title {
    display: none;
  }

  .search-section {
    padding: 16px;
    border-bottom: 1px solid #e9ecef;
  }

  .search-box {
    display: flex;
    gap: 8px;
    margin-bottom: 12px;
  }

  .search-box input {
    flex: 1;
    padding: 8px 12px;
    border: 1px solid #ddd;
    border-radius: 6px;
    font-size: 14px;
  }

  .search-btn {
    padding: 8px 12px;
    background: #007bff;
    color: white;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .search-results {
    background: #ffffff;
    border: 1px solid #e9ecef;
    border-radius: 6px;
    overflow: hidden;
  }

  .search-results-header {
    padding: 8px 12px;
    background: #f8f9fa;
    font-size: 12px;
    font-weight: 500;
    color: #666;
  }

  .search-result-item {
    width: 100%;
    padding: 8px 12px;
    border: none;
    background: none;
    text-align: left;
    cursor: pointer;
    font-size: 13px;
    border-bottom: 1px solid #f0f0f0;
  }

  .search-result-item:hover {
    background: #f8f9fa;
  }

  .thumbnails-container {
    flex: 1;
    overflow-y: auto;
    padding: 12px;
    display: grid;
    gap: 12px;
  }
  .sidebar.collapsed .thumbnails-container {
    display: none;
  }

  .bookmarks-section {
    flex: 1;
    display: flex;
    flex-direction: column;
    padding: 16px;
  }

  .bookmarks-header {
    margin-bottom: 12px;
  }

  .add-bookmark-btn {
    width: 100%;
    padding: 8px 12px;
    background: #28a745;
    color: white;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    font-size: 13px;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
  }

  .add-bookmark-btn:hover {
    background: #218838;
  }

  .bookmarks-list {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .bookmark-item {
    background: #ffffff;
    border: 1px solid #e9ecef;
    border-radius: 6px;
    overflow: hidden;
    display: flex;
  }

  .bookmark-link {
    flex: 1;
    padding: 12px;
    background: none;
    border: none;
    text-align: left;
    cursor: pointer;
    transition: background 0.2s ease;
  }

  .bookmark-link:hover {
    background: #f8f9fa;
  }

  .bookmark-label {
    font-weight: 500;
    color: #495057;
    font-size: 13px;
    margin-bottom: 2px;
    line-height: 1.3;
  }

  .bookmark-page {
    font-size: 11px;
    color: #6c757d;
  }

  .remove-bookmark-btn {
    padding: 8px;
    background: #dc3545;
    color: white;
    border: none;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background 0.2s ease;
  }

  .remove-bookmark-btn:hover {
    background: #c82333;
  }

  .no-bookmarks {
    text-align: center;
    color: #6c757d;
    font-size: 13px;
    padding: 24px 12px;
    line-height: 1.5;
  }

  .no-bookmarks small {
    font-size: 11px;
    opacity: 0.8;
  }

  .thumbnail {
    background: #ffffff;
    border: 2px solid #e9ecef;
    border-radius: 6px;
    padding: 8px;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .thumbnail:hover {
    border-color: #007bff;
    box-shadow: 0 2px 8px rgba(0, 123, 255, 0.15);
  }

  .thumbnail.active {
    border-color: #007bff;
    background: #f0f8ff;
  }
  .thumbnail-loading {
    aspect-ratio: 210/297;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #f8f9fa;
    border-radius: 4px;
  }

  .thumbnail-canvas-container {
    width: 100%;
    background: #fff;
    border-radius: 4px;
    overflow: hidden;
  }

  .thumbnail-canvas-container canvas {
    width: 100%;
    height: auto;
    display: block;
  }

  .thumbnail-label {
    text-align: center;
    font-size: 11px;
    color: #666;
    margin-top: 4px;
  }

  .main-content {
    flex: 1;
    display: flex;
    flex-direction: column;
    background: #f5f5f5;
  }

  .toolbar {
    background: #ffffff;
    border-bottom: 1px solid #e9ecef;
    padding: 12px 16px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 16px;
  }

  .toolbar-section {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .close-btn {
    background: #dc3545;
    color: white;
    border: none;
    border-radius: 6px;
    padding: 8px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .close-btn:hover {
    background: #c82333;
  }

  .file-name {
    font-weight: 500;
    color: #495057;
    max-width: 200px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 14px;
  }

  .page-input {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 14px;
  }

  .page-input input {
    width: 50px;
    padding: 4px 8px;
    border: 1px solid #ddd;
    border-radius: 4px;
    text-align: center;
  }

  .zoom-display {
    font-size: 14px;
    font-weight: 500;
    color: #495057;
    min-width: 50px;
    text-align: center;
  }

  .fit-buttons {
    display: flex;
    background: #f8f9fa;
    border-radius: 6px;
    overflow: hidden;
  }

  .fit-buttons button {
    padding: 6px 12px;
    border: none;
    background: none;
    cursor: pointer;
    font-size: 12px;
    font-weight: 500;
  }

  .fit-buttons button.active {
    background: #007bff;
    color: white;
  }

  .icon-btn {
    background: none;
    border: none;
    padding: 8px;
    cursor: pointer;
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #666;
  }
  .icon-btn:hover {
    background: #f0f0f0;
  }

  .icon-btn.active {
    background: #007bff;
    color: white;
  }

  button {
    background: #f8f9fa;
    color: #495057;
    border: 1px solid #e9ecef;
    border-radius: 6px;
    padding: 8px 12px;
    cursor: pointer;
    font-size: 14px;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.2s ease;
  }

  button:hover:not(:disabled) {
    background: #e9ecef;
    border-color: #dee2e6;
  }

  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .pdf-content {
    flex: 1;
    overflow: auto;
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding: 24px;
  }

  .pdf-canvas-container {
    background: #ffffff;
    border-radius: 8px;
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.1);
    padding: 20px;
  }

  .pdf-canvas {
    max-width: 100%;
    height: auto;
    display: block;
  }
  .loading-state, .error-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 400px;
    gap: 16px;
    color: #666;
    font-size: 16px;
  }

  .loading-progress {
    font-size: 14px;
    color: #999;
    opacity: 0.8;
  }

  .error-state {
    color: #dc3545;
  }

  .error-icon {
    font-size: 48px;
  }

  .error-message {
    text-align: center;
    font-size: 16px;
    max-width: 400px;
    line-height: 1.5;
  }

  .retry-btn {
    background: #28a745;
    color: white;
    border: none;
    border-radius: 6px;
    padding: 8px 16px;
    cursor: pointer;
    font-size: 14px;
  }

  .retry-btn:hover {
    background: #218838;
  }

  .spinner {
    width: 20px;
    height: 20px;
    border: 2px solid #f3f3f3;
    border-top: 2px solid #007bff;
    border-radius: 50%;
    animation: spin 1s linear infinite;
  }

  .spinner.large {
    width: 40px;
    height: 40px;
    border-width: 4px;
  }

  @keyframes spin {
    0% { transform: rotate(0deg); }
    100% { transform: rotate(360deg); }
  }

  /* 响应式设计 */
  @media (max-width: 1200px) {
    .modern-pdf-viewer {
      width: 70%;
    }
  }

  @media (max-width: 768px) {
    .modern-pdf-viewer {
      width: 100%;
    }
    
    .sidebar {
      width: 240px;
    }
    
    .toolbar {
      flex-wrap: wrap;
      gap: 8px;
    }
    
    .file-name {
      max-width: 120px;
    }
  }
</style>
