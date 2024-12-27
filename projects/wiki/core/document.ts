type Element = {
  text: string;
  link: string;
  bold: boolean;
  code: boolean;
  underline: boolean;
  italic: boolean;
  type:
    | 'block-quote'
    | 'bulleted-list'
    | 'numbered-list'
    | 'list-item'
    | 'heading-one'
    | 'heading-two'
    | 'heading-three'
    | 'heading-four'
    | 'heading-five'
    | 'heading-six';
  children: Element[];
};

const format = (elem: Element) => {
  const bold = elem.bold ? `<strong>${elem.text}</strong>` : elem.text;
  const code = elem.code ? `<em>${bold}</em>` : bold;
  const underline = elem.underline ? `<u>${code}</u>` : code;
  const italic = elem.italic ? `<i>${underline}</i>` : underline;
  const link = elem.link ? `<a href=${elem.link}>${italic}</a>` : italic;

  return link;
};

const toHtml = (doc: Element[]): string =>
  doc
    ?.map((element: Element) => {
      console.log(element)
      const formatted = element.children.map(format).join('');
      switch (element.type) {
        case 'block-quote':
          return `<blockquote>${formatted}</blockquote>`;
        case 'bulleted-list':
          return `<ul>${toHtml(element.children)}</ul>`;
        case 'numbered-list':
          return `<ol>${toHtml(element.children)}</ol>`;
        case 'heading-one':
          return `<h1>${formatted}</h1>`;
        case 'heading-two':
          return `<h2>${formatted}</h2>`;
        case 'heading-three':
          return `<h3>${formatted}</h3>`;
        case 'heading-four':
          return `<h4>${formatted}</h4>`;
        case 'heading-five':
          return `<h5>${formatted}</h5>`;
        case 'heading-six':
          return `<h6>${formatted}</h6>`;
        case 'list-item':
          return `<li>${formatted}</li>`;
        default:
          return `<p>${formatted}</p>`;
      }
    })
    .join('');

export { toHtml };
