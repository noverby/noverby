import { usePathname } from 'next/navigation';

const usePath = () => {
  return decodeURI(usePathname().slice(1));
};

export default usePath;
