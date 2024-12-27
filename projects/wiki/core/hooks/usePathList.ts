import { usePathname } from 'next/navigation';

const usePath = () => {
  const pathname = decodeURI(usePathname());
  return pathname === "/" ? [] : pathname.split("/").slice(1);
};

export default usePath;
