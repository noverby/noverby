type ErrorProps = {
	name: string;
	message: string;
	stack: string;
};

const ErrorPage = ({ name, message, stack }: ErrorProps) => (
	<>
		<p>An error occurred on client</p>
		<p>{`Name: ${name}`}</p>
		<p>{`Message: ${message}`}</p>
		<p>Stack:</p>
		<pre>
			<code>{stack}</code>
		</pre>
	</>
);

// eslint-disable-next-line functional/immutable-data
ErrorPage.getInitialProps = ({
	_res,
	err,
}: {
	_res: unknown;
	err: ErrorProps;
}) => ({
	name: err?.name,
	message: err?.message,
	stack: err?.stack,
});

export default ErrorPage;
