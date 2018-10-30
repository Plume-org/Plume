const articleContent = document.querySelector('#plume-editor')
const offset = articleContent.offsetHeight - articleContent.clientHeight

articleContent.addEventListener('keydown', () => {
  articleContent.style.height = 'auto'
  articleContent.style.height = `${articleContent.scrollHeight - offset}px`
})
