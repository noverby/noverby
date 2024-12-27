import { startTransition, useEffect, useState } from 'react';
import { useScreen } from 'hooks';

const PdfViewer = ({ file }: { file?: string }) => {
  const screen = useScreen();
  const [height, setHeight] = useState('0px');

  useEffect(() => {
    const scroll =
      document.querySelector('#scroll') ?? document.scrollingElement;
    startTransition(() => {
      setHeight(`${(scroll?.scrollHeight ?? 0) - (screen ? 100 : 210)}px`);
    });
  }, []);

  return file ? (
    <iframe
      width="100%"
      height={height}
      frameBorder="0"
      src={`https://docs.google.com/viewer?url=${encodeURIComponent(
        file
      )}&embedded=true`}
    />
  ) : null;
};

export default PdfViewer;
