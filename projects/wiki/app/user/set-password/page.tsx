"use client"
import React from 'react';
import { AuthForm, HeaderCard } from 'comps';
import { useAuthenticationStatus } from '@nhost/nextjs';
import { Container, Stack } from '@mui/system';
import { Avatar, CardContent, Typography } from '@mui/material';
import { MarkEmailRead } from '@mui/icons-material';

const Reset = () => {
  const { isAuthenticated } = useAuthenticationStatus();
  if (!isAuthenticated) {
    return <Container>
      <HeaderCard
        title="Tjek din email"
        avatar={
          <Avatar
            sx={{
              bgcolor: 'secondary.main',
            }}
          >
            <MarkEmailRead />
          </Avatar>
        }
      >
        <CardContent>
          <Stack spacing={1.5}>
            <Typography>
              Du skulle gerne have modtaget en email.
            </Typography>
            <Typography>Brug den til at nulstille dit kodeord.</Typography>
            <Typography>Tjek eventuelt om emailen er endt i spam.</Typography>
          </Stack>
        </CardContent>
      </HeaderCard>
    </Container>
  }

  return <AuthForm mode="set-password" />
}

export default Reset;
