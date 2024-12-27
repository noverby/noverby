import React, { useEffect, useState } from 'react';
import { useRouter } from 'next/navigation';
import {
  Container,
  TextField,
  Button,
  Avatar,
  Typography,
  Stack,
  CircularProgress,
  Box,
} from '@mui/material';
import { useSession } from 'hooks';
import { nhost } from 'nhost';
import { Email, HowToReg, LockReset, Login } from '@mui/icons-material';
import { useAuthenticationStatus } from '@nhost/nextjs';
import { FormEvent } from 'react';
import { ChangeEventHandler } from 'react';
import { client } from 'gql';

type Mode = 'login' | 'register' | 'reset-password' | 'set-password';

const LoginForm = ({ mode }: { mode: Mode }) => {
  const router = useRouter();
  const { isAuthenticated } = useAuthenticationStatus();
  const [_, setSession] = useSession();
  const [loading, setLoading] = useState(false);
  const [name, setName] = useState('');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [passwordRepeat, setPasswordRepeat] = useState('');
  const [errorName, setNameError] = useState('');
  const [errorEmail, setEmailError] = useState('');
  const [errorPassword, setPasswordError] = useState('');
  const [errorPasswordRepeat, setPasswordRepeatError] = useState('');

  useEffect(() => {
    if (
      ['login', 'register', 'reset-password'].includes(mode) &&
      !loading &&
      isAuthenticated
    ) {
      router.push('/');
    }
  }, [isAuthenticated, loading, mode]);

  const onNameChange: ChangeEventHandler<HTMLInputElement> = (e) => {
    const name = e.target.value;
    setName(name);
    if (name) {
      setNameError('');
    } else {
      setNameError('Mangler navn');
    }
  };

  const onEmailChange: ChangeEventHandler<HTMLInputElement> = (e) => {
    const email = e.target.value;
    setEmail(email);
    if (email) {
      setEmailError('');
      setPasswordError('');
    } else {
      setEmailError('Mangler email');
    }
  };

  const onPasswordChange: ChangeEventHandler<HTMLInputElement> = (e) => {
    const password = e.target.value;
    setPassword(password);
    if (password === passwordRepeat || passwordRepeat === '') {
      setEmailError('');
      setPasswordError('');
      setPasswordRepeatError('');
    } else {
      setPasswordError('Kodeord er ikke ens');
    }
  };

  const onPasswordRepeatChange: ChangeEventHandler<HTMLInputElement> = (e) => {
    const passwordRepeat = e.target.value;
    setPasswordRepeat(passwordRepeat);
    if (password === passwordRepeat || passwordRepeat === '') {
      setPasswordError('');
      setPasswordRepeatError('');
    } else {
      setPasswordRepeatError('Kodeord er ikke ens');
    }
  };

  const onLogin = async () => {
    if (email == '') {
      setEmailError('Mangler email');
      return;
    }
    if (password == '') {
      setPasswordError('Mangler kodeord');
      return;
    }
    const { error } = await nhost.auth.signIn({
      email: email.toLowerCase(),
      password,
    });
    if (error) {
      // Already logged-in
      if ([100].includes(error.status)) {
        router.push('/');
        return;
      }

      if (error.error === 'unverified-user') {
        nhost.auth.sendVerificationEmail({ email });
        setEmailError(
          'Email ikke verificeret. Tjek din indbakke. Evt. også spam.'
        );
        setLoading(false);
        return;
      }
      setEmailError('Forkert email eller kode');
      setPasswordError('Forkert email eller kode');
      setLoading(false);
      return;
    }

    // Set up
    setSession({
      timeDiff: undefined,
    });

    // Delete cache
    // eslint-disable-next-line functional/immutable-data
    client.cache.clear();

    await router.back();
  };

  const onRegister = async () => {
    if (name == '') {
      setNameError('Mangler navn');
      return;
    }
    if (email == '') {
      setEmailError('Mangler email');
      return;
    }
    if (password == '') {
      setPasswordError('Mangler kodeord');
      return;
    }
    if (passwordRepeat === '') {
      setPasswordRepeatError('Gentag kodeord');
      return;
    }
    const { error } = await nhost.auth.signUp({
      email: email.toLowerCase(),
      password,
      options: { displayName: name },
    });
    if (error) {
      switch (error.error) {
        case 'invalid-email':
          setEmailError('Email er ikke valid');
          break;
        case 'email-already-in-use':
          setEmailError('Email er allerede i brugt');
          break;
        default:
          setEmailError(error?.message);
      }
      return;
    }

    await router.push('/user/unverified');
  };

  const onSetPassword = async () => {
    if (password == '') {
      setPasswordError('Mangler kodeord');
      return;
    }
    if (passwordRepeat == '') {
      setPasswordRepeatError('Gentag kodeord');
      return;
    }
    const { error } = await nhost.auth.changePassword({
      newPassword: password,
    });

    if (error) {
      setPasswordError(error.message);
      return;
    }

    await router.push('/');
  };

  const onSendResetEmail = async () => {
    if (email == '') {
      setEmailError('Mangler email');
      return;
    }
    const { error } = await nhost.auth.resetPassword({
      email: email.toLowerCase(),
    });
    if (error) {
      switch (error.error) {
        case 'invalid-email':
          setEmailError('Invalid email');
          break;
        case 'user-not-found':
          setEmailError('Ingen bruger eksisterer med denne email');
          break;
        default:
          setEmailError(error.message);
      }
      return;
    }
    await router.push('/user/set-password');
  };

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setLoading(true);

    switch (mode) {
      case 'login':
        await onLogin();
        break;
      case 'register':
        await onRegister();
        break;
      case 'set-password':
        await onSetPassword();
        break;
      case 'reset-password':
        await onSendResetEmail();
        break;
    }
    setLoading(false);
  };

  const icon =
    mode === 'login' ? (
      <Login />
    ) : mode === 'register' ? (
      <HowToReg />
    ) : mode === 'reset-password' ? (
      <Email />
    ) : (
      <LockReset />
    );

  const text =
    mode === 'login'
      ? 'Log Ind'
      : mode === 'register'
      ? 'Registrer'
      : mode === 'reset-password'
      ? 'Nulstil Kodeord'
      : 'Sæt Kodeord';

  return (
    <Container sx={{ padding: 3 }} maxWidth="xs">
      <form onSubmit={handleSubmit}>
        <Stack spacing={2} alignItems="center">
          <Avatar sx={{ bgcolor: 'primary.main' }}>{icon}</Avatar>
          <Typography variant="h5">{text}</Typography>
          {mode === 'register' && (
            <TextField
              fullWidth
              error={!!errorName}
              helperText={errorName}
              label="Fulde navn"
              name="fullname"
              variant="outlined"
              onChange={onNameChange}
            />
          )}
          {mode !== 'set-password' && (
            <TextField
              fullWidth
              error={!!errorEmail}
              helperText={errorEmail}
              label="Email"
              autoComplete="username"
              name="email"
              variant="outlined"
              onChange={onEmailChange}
            />
          )}
          {mode !== 'reset-password' && (
            <TextField
              fullWidth
              error={!!errorPassword}
              helperText={errorPassword}
              label={mode == 'set-password' ? 'Nyt kodeord' : 'Kodeord'}
              autoComplete="current-password"
              name="password"
              type="password"
              variant="outlined"
              onChange={onPasswordChange}
            />
          )}
          {['register', 'set-password'].includes(mode) && (
            <TextField
              fullWidth
              error={!!errorPasswordRepeat}
              helperText={errorPasswordRepeat}
              label="Gentag Kodeord"
              name="password"
              type="password"
              variant="outlined"
              onChange={onPasswordRepeatChange}
            />
          )}
          <Box sx={{ position: 'relative', width: '100%' }}>
            <Button
              color="primary"
              fullWidth
              type="submit"
              variant="contained"
              startIcon={icon}
              disabled={
                loading ||
                !!(
                  errorName ||
                  errorEmail ||
                  errorPassword ||
                  errorPasswordRepeat
                )
              }
            >
              {text}
            </Button>

            {loading && (
              <CircularProgress
                size={24}
                sx={{
                  position: 'absolute',
                  top: '50%',
                  left: '50%',
                  marginTop: '-12px',
                  marginLeft: '-12px',
                }}
              />
            )}
          </Box>
          {['login'].includes(mode) && (
            <Button
              color="secondary"
              fullWidth
              variant="contained"
              startIcon={<HowToReg />}
              onClick={() => router.push('/user/register')}
            >
              Registrer
            </Button>
          )}
          {['login'].includes(mode) && (
            <Button
              color="secondary"
              fullWidth
              variant="contained"
              startIcon={<Email />}
              onClick={() => router.push('/user/reset-password')}
            >
              Nulstil kodeord
            </Button>
          )}
        </Stack>
      </form>
    </Container>
  );
};

export default LoginForm;
