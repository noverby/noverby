import { useRouter } from "next/router";
import { useEffect } from "react";

export default function Yt() {
  const router = useRouter();
  const match = router.asPath.match(/.*q=([^&]*)/)?.[1];

  useEffect(() => {
    router.push(`https://kagi.com${match ? `/search?q=${match}` : ""}`);
  }, [match, router]);
  return null;
}
