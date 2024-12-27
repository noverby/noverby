import { useSearchParams } from 'next/navigation';

const useScreen = () => {
  const params = useSearchParams();
  return params.get("app") === 'screen';
};

export default useScreen;
