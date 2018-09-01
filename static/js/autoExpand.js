function autosize(){
  const el = this;
  el.style.height = 'auto';
  el.style.height = (el.scrollHeight ) + 'px';
}

const articleContent = document.querySelector('#content');
let offset = 0;
let style = window.getComputedStyle(articleContent, null);

offset += parseInt(style['paddingTop']) + parseInt(style['paddingBottom']);
autosize.bind(articleContent)();
articleContent.addEventListener('keyup', autosize);

