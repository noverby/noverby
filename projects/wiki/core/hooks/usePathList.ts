import { useRouter } from 'next/router';

const usePathList = () => {
  const router = useRouter();
  const query = router.query;
  return Array.isArray(query.path)
    ? query.path
    : query.path
    ? [query.path]
    : [];
};

export default usePathList;
