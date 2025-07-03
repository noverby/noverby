import { useRouter } from 'next/router';

const usePath = () => {
  const router = useRouter();
  return decodeURI(router.asPath.slice(1));
};

export default usePath;
