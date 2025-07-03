import { useRouter } from 'next/router';

const useScreen = () => {
  const router = useRouter();
  return router.query.app === 'screen';
};

export default useScreen;
