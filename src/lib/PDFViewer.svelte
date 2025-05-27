<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  const { filePath, initialPage = 1, onClose } = $props<{ filePath: string; initialPage?: number; onClose: () => void}>();
  let pdfData = $state<string>("");
  let currentPage = $state(initialPage);
  let totalPages = $state(1);
  let loading = $state(true);
  let error = $state("");
  let pdfDoc: any = null;
  let scale = $state(1.2);
  let fitMode = $state<'width' | 'height' | 'page'>('page');
  let userScale = $state(1.2); // 用户手动设置的缩放
  let rotateAngle = $state(0);
  
  // 侧边栏状态
  let showSidebar = $state(true);
  let sidebarTab = $state<'thumbnails' | 'search' | 'bookmarks'>('thumbnails');
    // 缩略图状态
  let thumbnails = $state<Array<{
    pageNum: number;
    canvas: HTMLCanvasElement;
    loaded: boolean;
    aspectRatio: number;
  }>>([]);
  let thumbnailsContainer = $state<HTMLDivElement>();
  
  // 搜索状态
  let searchText = $state("");
  let searchResults = $state<Array<{ pageNum: number; matches: number; text: string }>>([]);
  let isSearching = $state(false);
  
  // 书签状态
  let bookmarks = $state<Array<{ pageNum: number; label: string }>>([]);
  async function waitForPageContainer(maxAttempts = 10): Promise<boolean> {
    for (let i = 0; i < maxAttempts; i++) {
      const container = document.getElementById('pdfPageContainer');
      if (container) {
        return true;
      }
      await new Promise(resolve => setTimeout(resolve, 100));
    }
    return false;
  }

  onMount(async () => {
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
      
      const containerReady = await waitForPageContainer();
      if (!containerReady) {
        error = "PDF容器初始化失败";
        return;
      }
      
      await renderPage(currentPage);
      await generateThumbnails();
    } catch (e) {
      console.error("Failed to load PDF:", e);
      error = `无法加载PDF文件: ${e}`;
      loading = false;
    }
  });async function generateThumbnails() {
    if (!pdfDoc) return;
    
    thumbnails = [];
    
    // First, get the dimensions of the first page to estimate aspect ratio
    let defaultAspectRatio = 3/4; // fallback
    try {
      const firstPage = await pdfDoc.getPage(1);
      const pageRotation = firstPage.rotate || 0;
      const firstViewport = firstPage.getViewport({ scale: 1, rotation: pageRotation });
      defaultAspectRatio = firstViewport.width / firstViewport.height;
    } catch (e) {
      console.error("Failed to get default aspect ratio:", e);
    }
    
    // 初始化所有缩略图为未加载状态
    for (let i = 1; i <= totalPages; i++) {
      thumbnails.push({
        pageNum: i,
        canvas: document.createElement('canvas'),
        loaded: false,
        aspectRatio: defaultAspectRatio // Use calculated default aspect ratio
      });
    }
    
    // 首先生成前10页的缩略图，确保快速显示
    const initialBatch = Math.min(totalPages, 10);
    for (let i = 1; i <= initialBatch; i++) {
      await generateSingleThumbnail(i - 1);
    }
    
    // 后台继续生成第11-50页的缩略图
    if (totalPages > initialBatch) {
      setTimeout(async () => {
        await generateRemainingThumbnails(initialBatch, Math.min(50, totalPages));
      }, 100);
    }
  }async function generateSingleThumbnail(index: number) {
    if (!pdfDoc || !thumbnails[index] || thumbnails[index].loaded) return;
    
    try {
      const pageNum = thumbnails[index].pageNum;
      const page = await pdfDoc.getPage(pageNum);
      
      // Get the page's natural rotation and viewport
      const pageRotation = page.rotate || 0;
      const baseViewport = page.getViewport({ scale: 1, rotation: pageRotation });
      
      // Calculate proper thumbnail scale - aim for max 200px width or height
      const maxDimension = 200;
      const thumbnailScale = Math.min(
        maxDimension / baseViewport.width,
        maxDimension / baseViewport.height
      );
      
      // Get viewport with proper scale and correct rotation
      const viewport = page.getViewport({ 
        scale: thumbnailScale,
        rotation: (pageRotation + rotateAngle) % 360  // 合并页面原始旋转和当前旋转角度
      });
      
      const canvas = thumbnails[index].canvas;
      canvas.width = viewport.width;
      canvas.height = viewport.height;
      
      const context = canvas.getContext('2d');
      if (context) {
        // Clear canvas
        context.clearRect(0, 0, canvas.width, canvas.height);
        
        // Set white background
        context.fillStyle = '#ffffff';
        context.fillRect(0, 0, canvas.width, canvas.height);
        
        // Set high quality rendering
        context.imageSmoothingEnabled = true;
        context.imageSmoothingQuality = 'high';
        
        // Render the page
        await page.render({
          canvasContext: context,
          viewport: viewport,
        }).promise;
      }
      
      // Calculate aspect ratio based on rendered dimensions
      thumbnails[index].loaded = true;
      thumbnails[index].aspectRatio = viewport.width / viewport.height;
      
      // 触发响应式更新
      thumbnails = [...thumbnails];
    } catch (e) {
      console.error(`Failed to generate thumbnail for page ${thumbnails[index].pageNum}:`, e);
    }
  }
  async function generateRemainingThumbnails(startIndex: number, endIndex?: number) {
    if (!pdfDoc) return;
    
    const end = endIndex !== undefined ? Math.min(endIndex, thumbnails.length) : thumbnails.length;
    
    // 分批生成剩余缩略图，避免阻塞UI
    const batchSize = 5;
    for (let i = startIndex; i < end; i += batchSize) {
      const batch = [];
      
      for (let j = i; j < Math.min(i + batchSize, end); j++) {
        batch.push(generateSingleThumbnail(j));
      }
      
      await Promise.all(batch);
      
      // 每批次之间短暂延迟，保持UI响应
      await new Promise(resolve => setTimeout(resolve, 50));
    }
  }

  // 懒加载缩略图 - 当缩略图进入视口时生成
  async function loadThumbnailOnDemand(index: number) {
    if (!thumbnails[index] || thumbnails[index].loaded) return;
    await generateSingleThumbnail(index);
  }  async function renderPage(pageNum: number) {
    if (!pdfDoc) return;

    try {
      const page = await pdfDoc.getPage(pageNum);
      let finalScale = userScale;
      
      // 获取页面容器
      const pageContainer = document.getElementById('pdfPageContainer');
      if (!pageContainer) return;
      
      // 根据适配模式计算最终的缩放比例
      if (fitMode === 'width') {
        const containerWidth = pageContainer.clientWidth - 40;
        const baseViewport = page.getViewport({ scale: 1, rotation: rotateAngle });
        finalScale = containerWidth / baseViewport.width;
      } else if (fitMode === 'height') {
        const containerHeight = pageContainer.clientHeight - 40;
        const baseViewport = page.getViewport({ scale: 1, rotation: rotateAngle });
        finalScale = containerHeight / baseViewport.height;
      }
      
      // 更新显示的scale值
      scale = finalScale;
      const viewport = page.getViewport({ scale: finalScale, rotation: rotateAngle });
      
      // 清空现有内容
      pageContainer.innerHTML = '';
      
      // 创建页面包装器
      const pageWrapper = document.createElement('div');
      pageWrapper.className = 'pdf-page-wrapper';
      pageWrapper.style.position = 'relative';
      pageWrapper.style.width = `${viewport.width}px`;
      pageWrapper.style.height = `${viewport.height}px`;
      pageWrapper.style.background = 'white';
      pageWrapper.style.borderRadius = '8px';
      pageWrapper.style.boxShadow = '0 4px 6px rgba(0, 0, 0, 0.1)';
      pageWrapper.style.overflow = 'hidden';
      
      // 创建Canvas层（用于渲染PDF的视觉内容）
      const canvas = document.createElement('canvas');
      canvas.width = viewport.width;
      canvas.height = viewport.height;
      canvas.style.position = 'absolute';
      canvas.style.top = '0';
      canvas.style.left = '0';
      canvas.style.width = '100%';
      canvas.style.height = '100%';
      canvas.style.zIndex = '1';
      
      // 创建文本层容器
      const textLayerDiv = document.createElement('div');
      textLayerDiv.className = 'textLayer';
      textLayerDiv.style.position = 'absolute';
      textLayerDiv.style.top = '0';
      textLayerDiv.style.left = '0';
      textLayerDiv.style.width = '100%';
      textLayerDiv.style.height = '100%';
      textLayerDiv.style.zIndex = '2';
      textLayerDiv.style.userSelect = 'text';
      textLayerDiv.style.pointerEvents = 'auto';
      
      // 将canvas和文本层添加到页面包装器
      pageWrapper.appendChild(canvas);
      pageWrapper.appendChild(textLayerDiv);
      pageContainer.appendChild(pageWrapper);
      
      // 渲染PDF到canvas
      const context = canvas.getContext("2d");
      if (context) {
        context.clearRect(0, 0, canvas.width, canvas.height);
        context.fillStyle = "#ffffff";
        context.fillRect(0, 0, canvas.width, canvas.height);
        
        await page.render({
          canvasContext: context,
          viewport: viewport,
        }).promise;
      }

      // 渲染文本层
      await renderTextLayer(page, viewport, textLayerDiv);
      
      if (error.includes("Canvas")) {
        error = "";
      }
    } catch (e) {
      console.error("Failed to render page:", e);
      error = `无法渲染PDF页面 ${pageNum}: ${e}`;
    }
  }  async function renderTextLayer(page: any, viewport: any, textLayerDiv: HTMLElement) {
    try {
      // 获取文本内容
      const textContent = await page.getTextContent();
      
      // 清空文本层容器
      textLayerDiv.innerHTML = '';
      
      // 动态导入pdfjs-dist
      const pdfjsLib = await import("pdfjs-dist");
      
      // 尝试使用PDF.js的TextLayer API
      try {
        if (pdfjsLib.TextLayer) {
          const textLayer = new pdfjsLib.TextLayer({
            textContentSource: textContent,
            container: textLayerDiv,
            viewport: viewport,
          });
          await textLayer.render();
        } else {
          // 回退到手动渲染
          await renderTextLayerManually(textContent, textLayerDiv, viewport);
        }
      } catch (e) {
        console.log("使用手动文本层渲染...", e);
        await renderTextLayerManually(textContent, textLayerDiv, viewport);
      }
    } catch (e) {
      console.error("文本层渲染失败:", e);
      // 文本层渲染失败不影响PDF显示，只是无法选择文本
    }
  }

  // 手动渲染文本层的改进版本
  async function renderTextLayerManually(textContent: any, container: HTMLElement, viewport: any) {
    try {
      // 清空容器
      container.innerHTML = '';
      
      // 设置容器样式
      container.style.position = 'absolute';
      container.style.left = '0px';
      container.style.top = '0px';
      container.style.right = '0px';
      container.style.bottom = '0px';
      container.style.overflow = 'hidden';
      container.style.opacity = '1';
      container.style.lineHeight = '1.0';
      container.style.fontSize = '1px';
      
      // 遍历文本项目并创建span元素
      textContent.items.forEach((item: any, index: number) => {
        if (!item.str || item.str.trim() === '') return;
        
        const span = document.createElement('span');
        span.textContent = item.str;
        span.style.position = 'absolute';
        span.style.whiteSpace = 'pre';
        span.style.color = 'transparent';
        span.style.cursor = 'text';
        span.style.userSelect = 'text';
        span.style.transformOrigin = '0% 0%';
        
        // 应用PDF.js的变换矩阵
        if (item.transform && item.transform.length >= 6) {
          const [scaleX, skewY, skewX, scaleY, translateX, translateY] = item.transform;
          
          // 设置位置
          span.style.left = `${translateX}px`;
          span.style.top = `${translateY}px`;
          
          // 设置字体大小（使用Y轴缩放作为字体大小）
          const fontSize = Math.abs(scaleY);
          span.style.fontSize = `${fontSize}px`;
          
          // 设置字体族（如果有的话）
          if (item.fontName) {
            span.style.fontFamily = item.fontName.replace(/[+]/g, ' ');
          }
          
          // 应用完整的变换矩阵
          if (scaleX !== 1 || skewY !== 0 || skewX !== 0 || scaleY !== 1) {
            const matrix = `matrix(${scaleX}, ${skewY}, ${skewX}, ${scaleY}, 0, 0)`;
            span.style.transform = matrix;
          }
          
          // 设置宽度和高度以提高选择精度
          if (item.width) {
            span.style.width = `${item.width}px`;
          }
          if (item.height) {
            span.style.height = `${Math.abs(scaleY)}px`;
          }
        }
        
        // 添加数据属性以便调试
        span.setAttribute('data-text-index', index.toString());
        
        container.appendChild(span);
      });
    } catch (e) {
      console.error("手动文本层渲染失败:", e);
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
  }  async function zoomIn() {
    userScale = Math.min(userScale * 1.2, 3.0);
    // 如果不是页面模式，切换到页面模式以使用用户设置的缩放
    if (fitMode !== 'page') {
      fitMode = 'page';
    }
    await renderPage(currentPage);
  }

  async function zoomOut() {
    userScale = Math.max(userScale / 1.2, 0.2);
    // 如果不是页面模式，切换到页面模式以使用用户设置的缩放
    if (fitMode !== 'page') {
      fitMode = 'page';
    }
    await renderPage(currentPage);
  }

  async function setFitMode(mode: 'width' | 'height' | 'page') {
    fitMode = mode;
    await renderPage(currentPage);
  }
  async function rotate() {
    rotateAngle = (rotateAngle + 90) % 360;
    await renderPage(currentPage);
    // 重新生成缩略图以反映新的旋转状态
    await generateThumbnails();
  }

  function toggleSidebar() {
    showSidebar = !showSidebar;
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
          searchResults.push({
            pageNum: i,
            matches: matches,
            text: text.substring(0, 200) + '...'
          });
        }
      }
    } catch (e) {
      console.error('Search failed:', e);
    } finally {
      isSearching = false;
    }
  }
  // 复制选中的文本到剪贴板
  async function copySelectedText() {
    try {
      const selection = window.getSelection();
      if (selection && selection.toString().trim()) {
        await navigator.clipboard.writeText(selection.toString());
        // 可以添加一个提示消息
        console.log('文本已复制到剪贴板');
      }
    } catch (e) {
      console.error('复制文本失败:', e);
    }
  }
  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      onClose();
    } else if (event.key === "ArrowLeft") {
      prevPage();
    } else if (event.key === "ArrowRight") {
      nextPage();
    } else if (event.key === "+" || event.key === "=") {
      zoomIn();
    } else if (event.key === "-") {
      zoomOut();
    } else if (event.key === "Home") {
      goToPage(1);
    } else if (event.key === "End") {
      goToPage(totalPages);
    } else if (event.ctrlKey && event.key === "c") {
      // Ctrl+C 复制选中文本
      copySelectedText();    } else if (event.ctrlKey && event.key === "a") {
      // Ctrl+A 全选当前页面文本
      event.preventDefault();
      const textLayer = document.querySelector('.textLayer');
      if (textLayer) {
        const selection = window.getSelection();
        const range = document.createRange();
        range.selectNodeContents(textLayer);
        selection?.removeAllRanges();
        selection?.addRange(range);
      }
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
  }  // 自定义指令：渲染缩略图到canvas
  function renderThumbnailCanvas(canvas: HTMLCanvasElement, sourceCanvas: HTMLCanvasElement) {
    if (!sourceCanvas || !canvas) return;
    
    function renderCanvas() {
      if (!sourceCanvas || sourceCanvas.width === 0 || sourceCanvas.height === 0) return;
      
      // Set the display canvas size to match the source canvas
      canvas.width = sourceCanvas.width;
      canvas.height = sourceCanvas.height;
      
      const ctx = canvas.getContext('2d');
      if (ctx) {
        // Set high quality rendering
        ctx.imageSmoothingEnabled = true;
        ctx.imageSmoothingQuality = 'high';
        
        // Clear the canvas
        ctx.clearRect(0, 0, canvas.width, canvas.height);
        
        // Set white background
        ctx.fillStyle = '#ffffff';
        ctx.fillRect(0, 0, canvas.width, canvas.height);
        
        // Draw the source canvas
        ctx.drawImage(sourceCanvas, 0, 0);
      }
    }
    
    // Initial render
    renderCanvas();
    
    return {
      update(newSourceCanvas: HTMLCanvasElement) {
        if (newSourceCanvas) {
          sourceCanvas = newSourceCanvas;
          renderCanvas();
        }
      }
    };
  }
  // 自定义指令：懒加载缩略图
  function lazyLoadThumbnail(node: HTMLElement, index: number) {
    const observer = new IntersectionObserver(
      async (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            const thumbnailIndex = index;
            const thumbnail = thumbnails[thumbnailIndex];
            
            if (thumbnail && !thumbnail.loaded) {
              // Add loading indicator
              const loadingEl = entry.target.querySelector('.thumbnail-loading') as HTMLElement;
              if (loadingEl) {
                loadingEl.style.display = 'flex';
              }
              
              try {
                await loadThumbnailOnDemand(thumbnailIndex);
              } catch (e) {
                console.error(`Failed to load thumbnail ${thumbnailIndex + 1}:`, e);
              } finally {
                // Hide loading indicator
                if (loadingEl) {
                  loadingEl.style.display = 'none';
                }
              }
            }
            
            // Stop observing this element once loaded
            if (thumbnail?.loaded) {
              observer.unobserve(entry.target);
            }
          }
        }
      },
      {
        root: thumbnailsContainer,
        rootMargin: '100px', // Load thumbnails when they're 100px away from viewport
        threshold: 0.1
      }
    );

    observer.observe(node);

    return {
      destroy() {
        observer.unobserve(node);
        observer.disconnect();
      }
    };
  }
</script>

<svelte:window on:keydown={handleKeydown} />

<div 
  class="pdf-viewer-overlay" 
  onclick={onClose}
  onkeydown={(e) => e.key === 'Enter' && onClose()} 
  role="button" 
  tabindex="0"
>
  <div 
    class="pdf-viewer-modal" 
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.stopPropagation()} 
    role="dialog" 
    tabindex="-1"
  >
    <!-- 顶部工具栏 -->
    <div class="pdf-header">
      <div class="header-left">
        <button onclick={onClose} class="close-btn" aria-label="关闭PDF查看器">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor">
            <path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/>
          </svg>
        </button>
        <div class="file-info">
          <h3 class="file-name">{filePath.split("\\").pop()}</h3>
          <span class="page-count">{totalPages} 页</span>
        </div>
      </div>
        <div class="header-center">
        <div class="page-navigation">
          <button onclick={prevPage} disabled={currentPage <= 1} class="nav-btn prev-btn" aria-label="上一页">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor">
              <path d="M15.41 16.59L10.83 12l4.58-4.59L14 6l-6 6 6 6 1.41-1.41z"/>
            </svg>
            <span class="nav-btn-label">上一页</span>
          </button>
          
          <div class="page-input-container">
            <div class="page-input-group">
              <span class="page-label">第</span>
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
                class="page-input"
              />
              <span class="page-separator">/ {totalPages}</span>
              <span class="page-label">页</span>
            </div>
            <div class="page-progress">
              <div class="progress-bar" style="width: {(currentPage / totalPages) * 100}%"></div>
            </div>
          </div>
          
          <button onclick={nextPage} disabled={currentPage >= totalPages} class="nav-btn next-btn" aria-label="下一页">
            <span class="nav-btn-label">下一页</span>
            <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor">
              <path d="M8.59 16.59L13.17 12 8.59 7.41 10 6l6 6-6 6-1.41-1.41z"/>
            </svg>
          </button>
        </div>
      </div>
      
      <div class="header-right">
        <div class="view-controls">
          <button onclick={toggleSidebar} class="icon-btn" class:active={showSidebar} aria-label="切换侧边栏">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
              <path d="M3 9h14V7H3v2zm0 4h14v-2H3v2zm0 4h14v-2H3v2zm16 0h2v-2h-2v2zm0-10v2h2V7h-2zm0 6h2v-2h-2v2z"/>
            </svg>
          </button>
            <button onclick={zoomOut} class="icon-btn" aria-label="缩小">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
              <path d="M19 13H5v-2h14v2z"/>
            </svg>
          </button>
          
          <span class="zoom-display">{Math.round(scale * 100)}%</span>
          
          <button onclick={zoomIn} class="icon-btn" aria-label="放大">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
              <path d="M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z"/>
            </svg>
          </button>
          
          <div class="fit-controls">
            <button 
              onclick={() => setFitMode('width')} 
              class="fit-btn" 
              class:active={fitMode === 'width'}
              title="适合宽度"
            >
              宽度
            </button>
            <button 
              onclick={() => setFitMode('height')} 
              class="fit-btn" 
              class:active={fitMode === 'height'}
              title="适合高度"
            >
              高度
            </button>
            <button 
              onclick={() => setFitMode('page')} 
              class="fit-btn" 
              class:active={fitMode === 'page'}
              title="适合页面"
            >
              页面
            </button>
          </div>
          
          <button onclick={rotate} class="icon-btn" aria-label="旋转90度">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
              <path d="M12 6v3l4-4-4-4v3c-4.42 0-8 3.58-8 8 0 1.57.46 3.03 1.24 4.26L6.7 14.8c-.45-.83-.7-1.79-.7-2.8 0-3.31 2.69-6 6-6zm6.76 1.74L17.3 9.2c.44.84.7 1.79.7 2.8 0 3.31-2.69 6-6 6v-3l-4 4 4 4v-3c4.42 0 8-3.58 8-8 0-1.57-.46-3.03-1.24-4.26z"/>
            </svg>
          </button>
            <button onclick={addBookmark} class="icon-btn" aria-label="添加书签">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
              <path d="M17 3H7c-1.1 0-1.99.9-1.99 2L5 21l7-3 7 3V5c0-1.1-.9-2-2-2z"/>
            </svg>
          </button>
          
          <button onclick={copySelectedText} class="icon-btn" aria-label="复制选中文本">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
              <path d="M16 1H4c-1.1 0-2 .9-2 2v14h2V3h12V1zm3 4H8c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h11c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm0 16H8V7h11v14z"/>
            </svg>
          </button>
        </div>
      </div>
    </div>

    <!-- 主要内容区域 -->
    <div class="pdf-main">
      <!-- 侧边栏 -->
      {#if showSidebar}
        <div class="pdf-sidebar">
          <div class="sidebar-tabs">
            <button 
              onclick={() => sidebarTab = 'thumbnails'} 
              class="tab-btn" 
              class:active={sidebarTab === 'thumbnails'}
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
                <path d="M4 6H2v14c0 1.1.9 2 2 2h14v-2H4V6zm16-4H8c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V4c0-1.1-.9-2-2-2zm-8 12.5v-9l6 4.5-6 4.5z"/>
              </svg>
              缩略图
            </button>
            <button 
              onclick={() => sidebarTab = 'search'} 
              class="tab-btn" 
              class:active={sidebarTab === 'search'}
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
                <path d="M15.5 14h-.79l-.28-.27C15.41 12.59 16 11.11 16 9.5 16 5.91 13.09 3 9.5 3S3 5.91 3 9.5 5.91 16 9.5 16c1.61 0 3.09-.59 4.23-1.57l.27.28v.79l5 4.99L20.49 19l-4.99-5zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 11.99 14 9.5 14z"/>
              </svg>
              搜索
            </button>
            <button 
              onclick={() => sidebarTab = 'bookmarks'} 
              class="tab-btn" 
              class:active={sidebarTab === 'bookmarks'}
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
                <path d="M17 3H7c-1.1 0-1.99.9-1.99 2L5 21l7-3 7 3V5c0-1.1-.9-2-2-2z"/>
              </svg>
              书签
            </button>
          </div>

          <div class="sidebar-content">
            {#if sidebarTab === 'thumbnails'}              <div class="thumbnails-grid" bind:this={thumbnailsContainer}>                {#each thumbnails as thumbnail, index}
                  <div 
                    class="thumbnail-item" 
                    class:active={thumbnail.pageNum === currentPage}
                    onclick={() => goToPage(thumbnail.pageNum)}
                    onkeydown={(e) => e.key === 'Enter' && goToPage(thumbnail.pageNum)}
                    role="button"
                    tabindex="0"
                    title="跳转到第 {thumbnail.pageNum} 页"
                    use:lazyLoadThumbnail={index}
                  >                    {#if thumbnail.loaded}
                      <div 
                        class="thumbnail-canvas-wrapper"
                        style="aspect-ratio: {thumbnail.aspectRatio}"
                      >
                        <canvas
                          data-page={thumbnail.pageNum}
                          style="width: 100%; height: 100%; object-fit: contain; background: white;"
                          use:renderThumbnailCanvas={thumbnail.canvas}
                        ></canvas>
                      </div>
                    {:else}
                      <div 
                        class="thumbnail-placeholder"
                        style="aspect-ratio: {thumbnail.aspectRatio}"
                      >
                        <div class="thumbnail-loading">
                          <div class="spinner small"></div>
                        </div>
                      </div>
                    {/if}
                    <div class="thumbnail-label">{thumbnail.pageNum}</div>
                  </div>
                {/each}
              </div>
            
            {:else if sidebarTab === 'search'}
              <div class="search-panel">
                <div class="search-input-group">
                  <input
                    type="text"
                    bind:value={searchText}
                    placeholder="搜索PDF内容..."
                    class="search-input"
                    onkeydown={(e) => {
                      if (e.key === 'Enter') {
                        searchInPDF();
                      }
                    }}
                  />
                  <button onclick={searchInPDF} disabled={isSearching} class="search-btn">
                    {#if isSearching}
                      <div class="spinner small"></div>
                    {:else}
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
                        <path d="M15.5 14h-.79l-.28-.27C15.41 12.59 16 11.11 16 9.5 16 5.91 13.09 3 9.5 3S3 5.91 3 9.5 5.91 16 9.5 16c1.61 0 3.09-.59 4.23-1.57l.27.28v.79l5 4.99L20.49 19l-4.99-5zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 11.99 14 9.5 14z"/>
                      </svg>
                    {/if}
                  </button>
                </div>
                
                {#if searchResults.length > 0}
                  <div class="search-results">
                    <div class="search-results-header">
                      找到 {searchResults.length} 个结果
                    </div>
                    {#each searchResults as result}
                      <div 
                        class="search-result-item"
                        onclick={() => goToPage(result.pageNum)}
                        onkeydown={(e) => e.key === 'Enter' && goToPage(result.pageNum)}
                        role="button"
                        tabindex="0"
                      >
                        <div class="result-page">第 {result.pageNum} 页</div>
                        <div class="result-matches">{result.matches} 个匹配</div>
                        <div class="result-preview">{result.text}</div>
                      </div>
                    {/each}
                  </div>
                {:else if searchText && !isSearching}
                  <div class="no-results">未找到匹配结果</div>
                {/if}
              </div>
            
            {:else if sidebarTab === 'bookmarks'}
              <div class="bookmarks-panel">
                <div class="bookmarks-header">
                  <button onclick={addBookmark} class="add-bookmark-btn">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
                      <path d="M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z"/>
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
                        <div class="bookmark-title">{bookmark.label}</div>
                        <div class="bookmark-page">第 {bookmark.pageNum} 页</div>
                      </button>
                      <button 
                        class="remove-bookmark-btn"
                        onclick={() => removeBookmark(bookmark.pageNum)}
                        aria-label="删除书签"
                      >
                        <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor">
                          <path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/>
                        </svg>
                      </button>
                    </div>
                  {/each}
                  
                  {#if bookmarks.length === 0}
                    <div class="empty-bookmarks">
                      <svg width="48" height="48" viewBox="0 0 24 24" fill="currentColor" opacity="0.3">
                        <path d="M17 3H7c-1.1 0-1.99.9-1.99 2L5 21l7-3 7 3V5c0-1.1-.9-2-2-2z"/>
                      </svg>
                      <p>暂无书签</p>
                      <small>点击顶部书签按钮为当前页面添加书签</small>
                    </div>
                  {/if}
                </div>
              </div>
            {/if}
          </div>
        </div>
      {/if}      <!-- PDF内容区域 -->
      <div 
        class="pdf-content" 
        onwheel={handleWheel}
        tabindex="-1"
        role="application"
        aria-label="PDF阅读器内容区域"
      >
        {#if loading}
          <div class="loading-state">
            <div class="spinner large"></div>
            <p>正在加载PDF文档...</p>
            <small>请稍候</small>
          </div>
        {:else if error}
          <div class="error-state">
            <div class="error-icon">⚠️</div>
            <p class="error-message">{error}</p>
            <button onclick={() => window.location.reload()} class="retry-btn">
              重新加载
            </button>
          </div>
        {:else}
          <div id="pdfPageContainer" class="pdf-page-container">
            <!-- PDF页面将在这里动态创建 -->
          </div>
        {/if}
      </div>
    </div>
  </div>
</div>

<style>
  /* 全局样式重置 */
  * {
    box-sizing: border-box;
  }

  /* 主容器 */
  .pdf-viewer-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: linear-gradient(135deg, rgba(0, 0, 0, 0.9), rgba(30, 30, 30, 0.95));
    backdrop-filter: blur(8px);
    display: flex;
    justify-content: center;
    align-items: center;
    z-index: 1000;
    padding: 20px;
    animation: fadeIn 0.3s ease-out;
  }

  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  .pdf-viewer-modal {
    background: linear-gradient(135deg, #ffffff, #fafbfc);
    border-radius: 16px;
    box-shadow: 
      0 25px 50px rgba(0, 0, 0, 0.25),
      0 0 0 1px rgba(255, 255, 255, 0.1);
    width: min(95vw, 1400px);
    height: min(95vh, 900px);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    transform: scale(0.95);
    animation: modalIn 0.3s ease-out forwards;
  }

  @keyframes modalIn {
    to {
      transform: scale(1);
    }
  }

  /* 顶部工具栏 */
  .pdf-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px 24px;
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
    color: white;
    border-radius: 16px 16px 0 0;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
  }

  .header-left {
    display: flex;
    align-items: center;
    gap: 16px;
    flex: 1;
    min-width: 0;
  }

  .close-btn {
    background: rgba(255, 255, 255, 0.15);
    color: white;
    border: none;
    border-radius: 8px;
    padding: 8px;
    cursor: pointer;
    transition: all 0.2s ease;
    display: flex;
    align-items: center;
    justify-content: center;
    backdrop-filter: blur(10px);
  }

  .close-btn:hover {
    background: rgba(255, 255, 255, 0.25);
    transform: translateY(-1px);
  }

  .file-info {
    min-width: 0;
    flex: 1;
  }

  .file-name {
    font-weight: 600;
    font-size: 18px;
    color: white;
    margin: 0 0 4px 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.1);
  }

  .page-count {
    font-size: 14px;
    color: rgba(255, 255, 255, 0.8);
    font-weight: 400;
  }
  .header-center {
    display: flex;
    align-items: center;
    margin: 0 24px;
  }

  .page-navigation {
    display: flex;
    align-items: center;
    gap: 16px;
    background: rgba(255, 255, 255, 0.12);
    padding: 12px 20px;
    border-radius: 16px;
    backdrop-filter: blur(12px);
    border: 1px solid rgba(255, 255, 255, 0.1);
    box-shadow: 
      0 8px 32px rgba(0, 0, 0, 0.1),
      inset 0 1px 0 rgba(255, 255, 255, 0.2);
  }

  .nav-btn {
    background: linear-gradient(135deg, rgba(255, 255, 255, 0.2), rgba(255, 255, 255, 0.1));
    color: white;
    border: none;
    border-radius: 12px;
    padding: 10px 16px;
    cursor: pointer;
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    font-weight: 500;
    backdrop-filter: blur(8px);
    border: 1px solid rgba(255, 255, 255, 0.15);
    box-shadow: 
      0 4px 16px rgba(0, 0, 0, 0.1),
      inset 0 1px 0 rgba(255, 255, 255, 0.2);
    position: relative;
    overflow: hidden;
  }

  .nav-btn:before {
    content: '';
    position: absolute;
    top: 0;
    left: -100%;
    width: 100%;
    height: 100%;
    background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.3), transparent);
    transition: left 0.5s;
  }

  .nav-btn:hover:not(:disabled):before {
    left: 100%;
  }

  .nav-btn:hover:not(:disabled) {
    background: linear-gradient(135deg, rgba(255, 255, 255, 0.3), rgba(255, 255, 255, 0.2));
    transform: translateY(-2px);
    box-shadow: 
      0 8px 24px rgba(0, 0, 0, 0.15),
      inset 0 1px 0 rgba(255, 255, 255, 0.3);
    border-color: rgba(255, 255, 255, 0.25);
  }

  .nav-btn:active:not(:disabled) {
    transform: translateY(-1px);
    transition-duration: 0.1s;
  }

  .nav-btn:disabled {
    background: rgba(255, 255, 255, 0.05);
    color: rgba(255, 255, 255, 0.3);
    cursor: not-allowed;
    transform: none;
    box-shadow: none;
    border-color: rgba(255, 255, 255, 0.05);
  }

  .nav-btn:disabled:before {
    display: none;
  }

  .nav-btn-label {
    font-weight: 600;
    letter-spacing: 0.025em;
  }

  .prev-btn svg {
    order: -1;
  }

  .next-btn svg {
    order: 1;
  }

  .page-input-container {
    display: flex;
    flex-direction: column;
    gap: 4px;
    position: relative;
  }

  .page-input-group {
    display: flex;
    align-items: center;
    gap: 8px;
    background: rgba(255, 255, 255, 0.95);
    padding: 8px 16px;
    border-radius: 12px;
    color: #333;
    box-shadow: 
      0 4px 16px rgba(0, 0, 0, 0.1),
      inset 0 1px 0 rgba(255, 255, 255, 0.7);
    border: 1px solid rgba(255, 255, 255, 0.3);
    backdrop-filter: blur(8px);
    transition: all 0.3s ease;
  }

  .page-input-group:focus-within {
    background: rgba(255, 255, 255, 1);
    box-shadow: 
      0 6px 20px rgba(0, 0, 0, 0.15),
      0 0 0 3px rgba(102, 126, 234, 0.2);
    transform: translateY(-1px);
  }

  .page-label {
    font-size: 12px;
    font-weight: 600;
    color: #64748b;
    white-space: nowrap;
  }

  .page-input {
    width: 60px;
    border: none;
    background: transparent;
    color: #1e293b;
    font-weight: 700;
    text-align: center;
    font-size: 16px;
    padding: 2px 4px;
    border-radius: 4px;
    transition: all 0.2s ease;
  }

  .page-input:focus {
    outline: none;
    background: rgba(102, 126, 234, 0.1);
    color: #667eea;
  }

  .page-separator {
    color: #94a3b8;
    font-size: 14px;
    font-weight: 600;
  }

  .page-progress {
    width: 100%;
    height: 3px;
    background: rgba(255, 255, 255, 0.3);
    border-radius: 2px;
    overflow: hidden;
    position: relative;
  }

  .progress-bar {
    height: 100%;
    background: linear-gradient(90deg, #667eea, #764ba2);
    border-radius: 2px;
    transition: width 0.3s cubic-bezier(0.4, 0, 0.2, 1);
    box-shadow: 0 0 8px rgba(102, 126, 234, 0.5);
    position: relative;
  }

  .progress-bar:after {
    content: '';
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.6), transparent);
    animation: progressShine 2s infinite;
  }

  @keyframes progressShine {
    0% { transform: translateX(-100%); }
    100% { transform: translateX(100%); }
  }
  .header-right {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .view-controls {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .icon-btn {
    background: linear-gradient(135deg, rgba(255, 255, 255, 0.18), rgba(255, 255, 255, 0.12));
    color: white;
    border: none;
    border-radius: 10px;
    padding: 10px;
    cursor: pointer;
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
    display: flex;
    align-items: center;
    justify-content: center;
    backdrop-filter: blur(12px);
    min-width: 40px;
    height: 40px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    box-shadow: 
      0 4px 12px rgba(0, 0, 0, 0.1),
      inset 0 1px 0 rgba(255, 255, 255, 0.2);
    position: relative;
    overflow: hidden;
  }

  .icon-btn:before {
    content: '';
    position: absolute;
    top: 0;
    left: -100%;
    width: 100%;
    height: 100%;
    background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.25), transparent);
    transition: left 0.4s;
  }

  .icon-btn:hover:not(:disabled):before {
    left: 100%;
  }

  .icon-btn:hover:not(:disabled) {
    background: linear-gradient(135deg, rgba(255, 255, 255, 0.28), rgba(255, 255, 255, 0.22));
    transform: translateY(-2px);
    box-shadow: 
      0 8px 20px rgba(0, 0, 0, 0.15),
      inset 0 1px 0 rgba(255, 255, 255, 0.3);
    border-color: rgba(255, 255, 255, 0.2);
  }

  .icon-btn:active:not(:disabled) {
    transform: translateY(-1px);
    transition-duration: 0.1s;
  }

  .icon-btn:disabled {
    background: rgba(255, 255, 255, 0.05);
    color: rgba(255, 255, 255, 0.3);
    cursor: not-allowed;
    transform: none;
    box-shadow: none;
    border-color: rgba(255, 255, 255, 0.05);
  }

  .icon-btn:disabled:before {
    display: none;
  }

  .icon-btn.active {
    background: linear-gradient(135deg, rgba(255, 255, 255, 0.35), rgba(255, 255, 255, 0.25));
    box-shadow: 
      0 6px 16px rgba(0, 0, 0, 0.12),
      inset 0 1px 0 rgba(255, 255, 255, 0.4);
    border-color: rgba(255, 255, 255, 0.25);
  }

  .zoom-display {
    background: linear-gradient(135deg, rgba(255, 255, 255, 0.15), rgba(255, 255, 255, 0.1));
    color: white;
    padding: 8px 14px;
    border-radius: 10px;
    font-size: 13px;
    font-weight: 700;
    min-width: 60px;
    text-align: center;
    backdrop-filter: blur(12px);
    border: 1px solid rgba(255, 255, 255, 0.1);
    box-shadow: 
      0 4px 12px rgba(0, 0, 0, 0.1),
      inset 0 1px 0 rgba(255, 255, 255, 0.2);
    letter-spacing: 0.025em;
  }

  .fit-controls {
    display: flex;
    background: linear-gradient(135deg, rgba(255, 255, 255, 0.15), rgba(255, 255, 255, 0.1));
    border-radius: 10px;
    overflow: hidden;
    backdrop-filter: blur(12px);
    border: 1px solid rgba(255, 255, 255, 0.1);
    box-shadow: 
      0 4px 12px rgba(0, 0, 0, 0.1),
      inset 0 1px 0 rgba(255, 255, 255, 0.2);
  }

  .fit-btn {
    background: transparent;
    color: white;
    border: none;
    padding: 8px 14px;
    cursor: pointer;
    font-size: 13px;
    font-weight: 600;
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
    white-space: nowrap;
    position: relative;
    overflow: hidden;
  }

  .fit-btn:before {
    content: '';
    position: absolute;
    top: 0;
    left: -100%;
    width: 100%;
    height: 100%;
    background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.2), transparent);
    transition: left 0.4s;
  }

  .fit-btn:hover:before {
    left: 100%;
  }

  .fit-btn:hover {
    background: rgba(255, 255, 255, 0.12);
  }

  .fit-btn.active {
    background: rgba(255, 255, 255, 0.25);
    font-weight: 700;
    color: #fff;
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.1);
  }

  /* 主内容区域 */
  .pdf-main {
    display: flex;
    flex: 1;
    overflow: hidden;
  }

  /* 侧边栏 */
  .pdf-sidebar {
    width: 280px;
    background: #f8fafc;
    border-right: 1px solid #e2e8f0;
    display: flex;
    flex-direction: column;
    box-shadow: 2px 0 8px rgba(0, 0, 0, 0.05);
  }

  .sidebar-tabs {
    display: flex;
    background: #fff;
    border-bottom: 1px solid #e2e8f0;
  }

  .tab-btn {
    flex: 1;
    background: transparent;
    border: none;
    padding: 12px 8px;
    cursor: pointer;
    font-size: 12px;
    font-weight: 500;
    color: #64748b;
    transition: all 0.2s ease;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    border-bottom: 2px solid transparent;
  }

  .tab-btn:hover {
    background: #f1f5f9;
    color: #475569;
  }

  .tab-btn.active {
    color: #667eea;
    border-bottom-color: #667eea;
    background: #f8fafc;
  }

  .sidebar-content {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  /* 缩略图网格 */
  .thumbnails-grid {
    padding: 16px;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(80px, 1fr));
    gap: 12px;
    overflow-y: auto;
    scrollbar-width: thin;
    scrollbar-color: #cbd5e1 transparent;
  }

  .thumbnails-grid::-webkit-scrollbar {
    width: 6px;
  }

  .thumbnails-grid::-webkit-scrollbar-track {
    background: transparent;
  }

  .thumbnails-grid::-webkit-scrollbar-thumb {
    background: #cbd5e1;
    border-radius: 3px;
  }

  .thumbnail-item {
    background: white;
    border-radius: 8px;
    padding: 8px;
    cursor: pointer;
    transition: all 0.2s ease;
    border: 2px solid transparent;
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
  }

  .thumbnail-item:hover {
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
    transform: translateY(-1px);
  }

  .thumbnail-item.active {
    border-color: #667eea;
    box-shadow: 0 4px 12px rgba(102, 126, 234, 0.3);
  }  .thumbnail-canvas-wrapper {
    position: relative;
    width: 100%;
    overflow: hidden;
    border-radius: 4px;
    background: #f8fafc;
  }

  .thumbnail-canvas-wrapper canvas {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
  }  .thumbnail-placeholder {
    width: 100%;
    background: linear-gradient(135deg, #f1f5f9 0%, #e2e8f0 100%);
    border-radius: 4px;
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    border: 1px solid #e2e8f0;
    min-height: 80px;
  }

  .thumbnail-loading {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
  }
  .thumbnail-canvas-wrapper {
    width: 100%;
    position: relative;
    border-radius: 4px;
    overflow: hidden;
    border: 1px solid #e2e8f0;
    background: white;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .thumbnail-canvas-wrapper canvas {
    max-width: 100%;
    max-height: 100%;
    display: block;
    background: white;
    border-radius: 3px;
  }

  .thumbnail-label {
    text-align: center;
    font-size: 11px;
    font-weight: 600;
    color: #475569;
    margin-top: 4px;
  }

  /* 搜索面板 */
  .search-panel {
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    height: 100%;
  }

  .search-input-group {
    display: flex;
    gap: 8px;
  }

  .search-input {
    flex: 1;
    padding: 10px 12px;
    border: 1px solid #d1d5db;
    border-radius: 8px;
    font-size: 14px;
    transition: all 0.2s ease;
    background: white;
  }

  .search-input:focus {
    outline: none;
    border-color: #667eea;
    box-shadow: 0 0 0 3px rgba(102, 126, 234, 0.1);
  }

  .search-btn {
    background: #667eea;
    color: white;
    border: none;
    border-radius: 8px;
    padding: 10px 16px;
    cursor: pointer;
    transition: all 0.2s ease;
    display: flex;
    align-items: center;
    justify-content: center;
    min-width: 44px;
  }

  .search-btn:hover:not(:disabled) {
    background: #5a67d8;
    transform: translateY(-1px);
  }

  .search-btn:disabled {
    background: #9ca3af;
    cursor: not-allowed;
  }

  .search-results {
    flex: 1;
    overflow-y: auto;
    scrollbar-width: thin;
    scrollbar-color: #cbd5e1 transparent;
  }

  .search-results::-webkit-scrollbar {
    width: 6px;
  }

  .search-results::-webkit-scrollbar-track {
    background: transparent;
  }

  .search-results::-webkit-scrollbar-thumb {
    background: #cbd5e1;
    border-radius: 3px;
  }

  .search-results-header {
    font-size: 14px;
    font-weight: 600;
    color: #374151;
    margin-bottom: 12px;
    padding-bottom: 8px;
    border-bottom: 1px solid #e5e7eb;
  }

  .search-result-item {
    background: white;
    border-radius: 8px;
    padding: 12px;
    margin-bottom: 8px;
    cursor: pointer;
    transition: all 0.2s ease;
    border: 1px solid #e5e7eb;
  }

  .search-result-item:hover {
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
    transform: translateY(-1px);
    border-color: #667eea;
  }

  .result-page {
    font-size: 12px;
    font-weight: 600;
    color: #667eea;
    margin-bottom: 4px;
  }

  .result-matches {
    font-size: 11px;
    color: #6b7280;
    margin-bottom: 6px;
  }
  .result-preview {
    font-size: 12px;
    color: #374151;
    line-height: 1.4;
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }

  .no-results {
    text-align: center;
    color: #6b7280;
    font-size: 14px;
    padding: 32px 16px;
  }

  /* 书签面板 */
  .bookmarks-panel {
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    height: 100%;
  }

  .bookmarks-header {
    border-bottom: 1px solid #e5e7eb;
    padding-bottom: 12px;
  }

  .add-bookmark-btn {
    background: #10b981;
    color: white;
    border: none;
    border-radius: 8px;
    padding: 8px 12px;
    cursor: pointer;
    font-size: 12px;
    font-weight: 500;
    transition: all 0.2s ease;
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .add-bookmark-btn:hover {
    background: #059669;
    transform: translateY(-1px);
  }

  .bookmarks-list {
    flex: 1;
    overflow-y: auto;
    scrollbar-width: thin;
    scrollbar-color: #cbd5e1 transparent;
  }

  .bookmarks-list::-webkit-scrollbar {
    width: 6px;
  }

  .bookmarks-list::-webkit-scrollbar-track {
    background: transparent;
  }

  .bookmarks-list::-webkit-scrollbar-thumb {
    background: #cbd5e1;
    border-radius: 3px;
  }

  .bookmark-item {
    display: flex;
    background: white;
    border-radius: 8px;
    margin-bottom: 8px;
    border: 1px solid #e5e7eb;
    overflow: hidden;
    transition: all 0.2s ease;
  }

  .bookmark-item:hover {
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
  }

  .bookmark-link {
    flex: 1;
    background: transparent;
    border: none;
    padding: 12px;
    cursor: pointer;
    text-align: left;
    transition: all 0.2s ease;
  }

  .bookmark-link:hover {
    background: #f8fafc;
  }

  .bookmark-title {
    font-size: 13px;
    font-weight: 600;
    color: #374151;
    margin-bottom: 4px;
  }

  .bookmark-page {
    font-size: 11px;
    color: #6b7280;
  }

  .remove-bookmark-btn {
    background: transparent;
    border: none;
    padding: 12px 16px;
    cursor: pointer;
    color: #ef4444;
    transition: all 0.2s ease;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .remove-bookmark-btn:hover {
    background: #fef2f2;
    color: #dc2626;
  }

  .empty-bookmarks {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    padding: 48px 24px;
    color: #6b7280;
  }

  .empty-bookmarks p {
    margin: 12px 0 8px 0;
    font-weight: 500;
  }

  .empty-bookmarks small {
    font-size: 12px;
    line-height: 1.4;
  }

  /* PDF内容区域 */
  .pdf-content {
    flex: 1;
    overflow: auto;
    padding: 24px;
    display: flex;
    justify-content: center;
    align-items: flex-start;
    background: linear-gradient(135deg, #f8fafc 0%, #e2e8f0 100%);
    scrollbar-width: thin;
    scrollbar-color: #cbd5e1 transparent;
  }

  .pdf-content::-webkit-scrollbar {
    width: 8px;
    height: 8px;
  }

  .pdf-content::-webkit-scrollbar-track {
    background: transparent;
  }

  .pdf-content::-webkit-scrollbar-thumb {
    background: #cbd5e1;
    border-radius: 4px;
  }
  .pdf-canvas-container {
    background: white;
    border-radius: 12px;
    padding: 20px;
    box-shadow: 
      0 20px 25px -5px rgba(0, 0, 0, 0.1),
      0 10px 10px -5px rgba(0, 0, 0, 0.04);
    transition: all 0.3s ease;
  }

  .pdf-canvas-container:hover {
    box-shadow: 
      0 25px 50px -12px rgba(0, 0, 0, 0.15),
      0 20px 20px -5px rgba(0, 0, 0, 0.06);
  }

  .pdf-canvas {
    max-width: 100%;
    max-height: 100%;
    display: block;
    border-radius: 8px;
    box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1);
  }

  /* 新的PDF页面容器样式 */
  .pdf-page-container {
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding: 20px;
    min-height: 100%;
  }

  .pdf-page-wrapper {
    position: relative;
    background: white;
    border-radius: 12px;
    box-shadow: 
      0 20px 25px -5px rgba(0, 0, 0, 0.1),
      0 10px 10px -5px rgba(0, 0, 0, 0.04);
    transition: all 0.3s ease;
    overflow: hidden;
  }

  .pdf-page-wrapper:hover {
    box-shadow: 
      0 25px 50px -12px rgba(0, 0, 0, 0.15),
      0 20px 20px -5px rgba(0, 0, 0, 0.06);
  }

  /* 加载和错误状态 */
  .loading-state, .error-state {
    display: flex;
    flex-direction: column;
    justify-content: center;
    align-items: center;
    height: 100%;
    text-align: center;
    padding: 48px 24px;
  }

  .loading-state p {
    font-size: 18px;
    font-weight: 600;
    color: #374151;
    margin: 16px 0 8px 0;
  }

  .loading-state small {
    color: #6b7280;
    font-size: 14px;
  }

  .error-state {
    color: #ef4444;
  }

  .error-icon {
    font-size: 48px;
    margin-bottom: 16px;
  }

  .error-message {
    font-size: 16px;
    font-weight: 500;
    margin-bottom: 16px;
    max-width: 400px;
  }

  .retry-btn {
    background: #10b981;
    color: white;
    border: none;
    border-radius: 8px;
    padding: 12px 24px;
    cursor: pointer;
    font-size: 14px;
    font-weight: 500;
    transition: all 0.2s ease;
  }

  .retry-btn:hover {
    background: #059669;
    transform: translateY(-1px);
  }

  /* 加载动画 */
  .spinner {
    border: 3px solid #f3f4f6;
    border-radius: 50%;
    border-top: 3px solid #667eea;
    animation: spin 1s linear infinite;
  }

  .spinner.small {
    width: 16px;
    height: 16px;
    border-width: 2px;
  }

  .spinner.large {
    width: 40px;
    height: 40px;
  }

  @keyframes spin {
    0% { transform: rotate(0deg); }
    100% { transform: rotate(360deg); }
  }

  /* 响应式设计 */
  @media (max-width: 768px) {
    .pdf-viewer {
      height: calc(100vh - env(safe-area-inset-top) - env(safe-area-inset-bottom));
    }

    .pdf-toolbar {
      padding: 8px 12px;
      flex-wrap: wrap;
      gap: 8px;
    }

    .pdf-toolbar h2 {
      font-size: 14px;
      margin-right: 8px;
    }

    .toolbar-group {
      gap: 4px;
    }

    .toolbar-btn {
      padding: 6px 8px;
      font-size: 12px;
      min-width: 32px;
    }

    .sidebar {
      width: 280px;
    }

    .pdf-content {
      padding: 12px;
    }

    .pdf-canvas-container {
      padding: 12px;
    }

    .thumbnail-grid {
      grid-template-columns: repeat(auto-fill, minmax(80px, 1fr));
      gap: 8px;
    }

    .search-panel, .bookmarks-panel {
      padding: 12px;
    }
  }

  /* 文本层样式 - 支持文本选择 */
  :global(.textLayer) {
    position: absolute;
    left: 0;
    top: 0;
    right: 0;
    bottom: 0;
    overflow: hidden;
    opacity: 1;
    line-height: 1.0;
    pointer-events: auto;
    user-select: text;
    -webkit-user-select: text;
    -moz-user-select: text;
    -ms-user-select: text;
  }

  :global(.textLayer span) {
    color: transparent;
    position: absolute;
    white-space: pre;
    cursor: text;
    transform-origin: 0% 0%;
  }

  :global(.textLayer span::selection) {
    background: rgba(0, 100, 255, 0.3);
  }

  :global(.textLayer span::-moz-selection) {
    background: rgba(0, 100, 255, 0.3);
  }

  /* 搜索高亮样式 */
  :global(.textLayer .highlight) {
    background: rgba(255, 255, 0, 0.6);
    border-radius: 2px;
  }

  :global(.textLayer .highlight.selected) {
    background: rgba(255, 165, 0, 0.6);
  }
  @media (max-width: 768px) {
    .pdf-viewer-modal {
      width: 100vw;
      height: 100vh;
      border-radius: 0;
    }

    .pdf-header {
      padding: 12px 16px;
      border-radius: 0;
    }

    .header-center {
      margin: 0 16px;
    }

    .file-name {
      font-size: 16px;
    }

    .pdf-sidebar {
      width: 240px;
    }

    .thumbnails-grid {
      grid-template-columns: repeat(auto-fill, minmax(60px, 1fr));
      gap: 8px;
      padding: 12px;
    }

    .pdf-content {
      padding: 16px;
    }
  }

  @media (max-width: 480px) {
    .pdf-main {
      flex-direction: column;
    }

    .pdf-sidebar {
      width: 100%;
      height: 200px;
      border-right: none;
      border-bottom: 1px solid #e2e8f0;
    }

    .thumbnails-grid {
      grid-template-columns: repeat(auto-fill, minmax(50px, 1fr));
    }
  }
</style>
