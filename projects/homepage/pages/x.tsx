import { useRouter } from "next/router";
import { useEffect } from "react";

export default function X() {
  const router = useRouter();
  const match = router.asPath.match(/.*(x|twitter)\.com(.*)/)?.[2];  
  useEffect(() => {
      if (!match) return;
      router.push(`https://xcancel.com${match}`);
  }, [match, router]);
  return null;
}

