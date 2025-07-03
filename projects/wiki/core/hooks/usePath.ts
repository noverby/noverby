import { useRouter } from 'next/router';

const usePath = () => {
  const router = useRouter();
  return router.asPath.slice(1);
};

export default usePath;
