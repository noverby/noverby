"use client"
import { Loader } from 'comps';
import { useRouter } from 'next/router';
import { startTransition, useEffect } from 'react';

const Index = () => {
  const router = useRouter();

  useEffect(() => {
    if (router.query.type === 'passwordReset') {
      startTransition(() => {
        router.push('/user/set-password');
      });
    }
  }, [router.query.type]);

  return <Loader app="home" />;
};

export default Index;
