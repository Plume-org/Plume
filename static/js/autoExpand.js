function autosize (el) {
  el.style.height = 'auto'
  el.style.height = `${el.scrollHeight}px`
}

const articleContent = document.querySelector('#plume-editor')
articleContent.addEventListener('keyup', () => autosize(articleContent))
