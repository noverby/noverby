import { useRouter } from 'next/router';
import { useEffect } from 'react';

const P404 = () => {
  const router = useRouter();

  useEffect(() => {
    router.push(router.asPath);
  }, [router.asPath]);

  return (
    <>
      <p>404: Path not found!</p>
      <p>This should not happen...</p>
    </>
  );
};

export default P404;
