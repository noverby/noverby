"use client"
type Error = {
  name: string;
  message: string;
  stack: string;
};

const Error = ({ name, message, stack }: Error) => (
  <>
    <p>An error occurred on client</p>
    <p>{`Name: ${name}`}</p>
    <p>{`Message: ${message}`}</p>
    <p>Stack:</p>
    {stack?.split('\n').map((line, index: number) => (
      <p key={index}>{line}</p>
    ))}
  </>
);

// eslint-disable-next-line functional/immutable-data
Error.getInitialProps = ({ res, err }: { res: unknown; err: Error }) => ({
  name: err?.name,
  message: err?.message,
  stack: err?.stack,
});

export default Error;
