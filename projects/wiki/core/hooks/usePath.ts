import { useRouter } from 'next/router';

const usePath = () => {
  const router = useRouter();
  return decodeURI(router.asPath.slice(1).split('?').slice(0, 1).join(''));
};

export default usePath;
