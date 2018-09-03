function autosize () {
  const el = this
  el.style.height = 'auto'
  el.style.height = `${el.scrollHeight}px`
}

const articleContent = document.querySelector('#content')
autosize.bind(articleContent)()
articleContent.addEventListener('keyup', autosize)
