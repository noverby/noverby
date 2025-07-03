import { Html, Head, Main, NextScript } from 'next/document';

const Document = () => (
  <Html lang="da">
    <Head>
      <link
        rel="stylesheet"
        href="https://api.fonts.coollabs.io/css?family=Roboto:300,400,500,700&display=swap"
      />
    </Head>
    <body>
      <Main />
      <NextScript />
    </body>
  </Html>
);

export default Document;