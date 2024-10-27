"use client"
import { Loader } from 'comps';
import { useRouter, useSearchParams } from 'next/navigation';
import { startTransition, useEffect } from 'react';

const Index = () => {
  const router = useRouter();
  const params = useSearchParams();

  useEffect(() => {
    if (params.get("type") === 'passwordReset') {
      startTransition(() => {
        router.push('/user/set-password');
      });
    }
  }, [params.get("type")]);

  return <Loader app="home" />;
};

export default Index;
