import { useRouter } from "next/router";
import { useEffect } from "react";

export default function Yt() {
  const router = useRouter();
  const regex = /.*q=([^&]*)/;
  const res = router.asPath.match(regex);

  useEffect(() => {
    const query = res?.[1];
    const url = query ? `https://kagi.com/search?q=${query}` : "https://kagi.com";
    router.push(url);
  }, [res, router]);
  return null;
}
