import { Head, Html, Main, NextScript } from "next/document";
import Script from "next/script";

const Document = () => (
	<Html lang="da">
		<Head>
			<link
				rel="stylesheet"
				href="https://api.fonts.coollabs.io/css?family=Roboto:300,400,500,700&display=swap"
			/>
			<Script
				src="https://cdn.counter.dev/script.js"
				data-id="3666fffc-f382-494e-8be9-1bb2902e3e0d"
				data-utcoffset="2"
			/>
		</Head>
		<body>
			<Main />
			<NextScript />
		</body>
	</Html>
);

export default Document;
