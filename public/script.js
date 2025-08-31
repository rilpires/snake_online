// ============================================================================
// VARIÁVEIS GLOBAIS
// ============================================================================

let ws = null;
let isConnected = false;
let gameState = null;
let isUpdatingSpeedFromServer = false; // Flag para evitar loop de updates

const canvas = document.getElementById('gameCanvas');
const ctx = canvas.getContext('2d');
const cellSize = 20; // 640/32 = 20 pixels por célula

// Elementos DOM
const statusDiv = document.getElementById('status');
const scoreSpan = document.getElementById('score');
const connectBtn = document.getElementById('connectBtn');
const resetBtn = document.getElementById('resetBtn');
const logsDiv = document.getElementById('logs');
const gameIdInput = document.getElementById('gameId');
const gameWidthInput = document.getElementById('gameWidth');
const gameHeightInput = document.getElementById('gameHeight');
const gameSpeedSlider = document.getElementById('gameSpeed');
const speedValueSpan = document.getElementById('speedValue');

// Elementos do modal e highscores
const gameOverModal = document.getElementById('gameOverModal');
const finalScoreSpan = document.getElementById('finalScore');
const usernameInput = document.getElementById('usernameInput');
const submitScoreBtn = document.getElementById('submitScoreBtn');
const skipScoreBtn = document.getElementById('skipScoreBtn');
const highscoresSidebar = document.getElementById('highscoresSidebar');
const highscoresList = document.getElementById('highscoresList');

const upBtn = document.getElementById('upBtn');
const downBtn = document.getElementById('downBtn');
const leftBtn = document.getElementById('leftBtn');
const rightBtn = document.getElementById('rightBtn');

// ============================================================================
// FUNÇÕES DE LOG
// ============================================================================

function addLog(message, type = 'info') {
    const timestamp = new Date().toLocaleTimeString();
    const logEntry = document.createElement('div');
    logEntry.className = `log-entry log-${type}`;
    logEntry.innerHTML = `<strong>[${timestamp}]</strong> ${message}`;
    
    logsDiv.appendChild(logEntry);
    logsDiv.scrollTop = logsDiv.scrollHeight;
    
    // Mantém apenas últimas 50 mensagens
    while (logsDiv.children.length > 50) {
        logsDiv.removeChild(logsDiv.firstChild);
    }

    // also console log
    console.log(message);
}

function updateSpeedDisplay() {
    speedValueSpan.textContent = gameSpeedSlider.value + 'ms';
}

function sendSpeedChange() {
    if (!isConnected || isUpdatingSpeedFromServer) return;
    
    const interval = parseInt(gameSpeedSlider.value);
    sendMessage({
        type: 'set_speed',
        interval: interval
    });
    
    addLog(`Velocidade alterada para ${interval}ms`, 'info');
}

function updateSpeedFromServer(interval) {
    isUpdatingSpeedFromServer = true;
    gameSpeedSlider.value = interval;
    updateSpeedDisplay();
    isUpdatingSpeedFromServer = false;
}

function showGameOverModal(score) {
    finalScoreSpan.textContent = score;
    usernameInput.value = '';
    gameOverModal.style.display = 'flex';
    
    // Foca no input após um pequeno delay para a animação
    setTimeout(() => {
        usernameInput.focus();
    }, 300);
}

function hideGameOverModal() {
    gameOverModal.style.display = 'none';
}

function submitUsername() {
    const username = usernameInput.value.trim();
    if (username.length === 0) {
        alert('Por favor, digite um nome!');
        return;
    }
    
    if (username.length > 140) {
        alert('Nome muito longo! Máximo 140 caracteres.');
        return;
    }
    
    sendMessage({
        type: 'username',
        username: username
    });
    
    addLog(`Nome enviado: ${username}`, 'info');
    hideGameOverModal();
}

function skipScore() {
    addLog('Pontuação não enviada', 'info');
    hideGameOverModal();
}

function renderHighscores(highscores) {
    highscoresList.innerHTML = '';
    
    // Converte o objeto em array e ordena
    const sortedScores = Object.entries(highscores)
        .map(([rank, data]) => ({ rank: parseInt(rank), ...data }))
        .sort((a, b) => a.rank - b.rank);
    
    sortedScores.forEach((entry) => {
        const item = document.createElement('div');
        item.className = 'highscore-item';
        
        const rankSpan = document.createElement('span');
        rankSpan.className = 'highscore-rank';
        rankSpan.textContent = `#${entry.rank}`;
        
        const nameSpan = document.createElement('span');
        nameSpan.className = 'highscore-name';
        nameSpan.textContent = entry.username;
        
        const scoreSpan = document.createElement('span');
        scoreSpan.className = 'highscore-score';
        scoreSpan.textContent = entry.scores;
        
        item.appendChild(rankSpan);
        item.appendChild(nameSpan);
        item.appendChild(scoreSpan);
        
        highscoresList.appendChild(item);
    });
    
    // Mostra a sidebar
    highscoresSidebar.style.display = 'block';
}

// ============================================================================
// FUNÇÕES DE CONFIGURAÇÃO
// ============================================================================

function getGameSize() {
    const width = parseInt(gameWidthInput.value) || 32;
    const height = parseInt(gameHeightInput.value) || 32;
    
    // Limita os valores entre 10 e 50
    const clampedWidth = Math.max(10, Math.min(50, width));
    const clampedHeight = Math.max(10, Math.min(50, height));
    
    // Atualiza os campos se os valores foram alterados
    if (width !== clampedWidth) gameWidthInput.value = clampedWidth;
    if (height !== clampedHeight) gameHeightInput.value = clampedHeight;
    
    return { width: clampedWidth, height: clampedHeight };
}

function getJoinGameMessage() {
    const message = {
        type: 'join_game'
    };
    
    // Adiciona game_id se fornecido
    const gameId = gameIdInput.value.trim();
    if (gameId) {
        message.game_id = gameId;
    }
    
    // Adiciona size se valores diferentes do padrão ou se game_id foi fornecido
    const gameSize = getGameSize();
    if (gameSize.width !== 32 || gameSize.height !== 32 || gameId) {
        message.size = gameSize;
    }
    
    return message;
}

// ============================================================================
// FUNÇÕES DE CONEXÃO WebSocket
// ============================================================================

function connect() {
    if (isConnected) return;
    
    addLog('Conectando ao servidor...', 'info');
    ws = new WebSocket('ws://127.0.0.1:8080');
    
    ws.onopen = function() {
        addLog('Conectado com sucesso!', 'received');
        setConnectedState(true);
        
        // Entra no jogo automaticamente com as configurações especificadas
        sendMessage(getJoinGameMessage());
    };
    
    ws.onmessage = function(event) {
        try {
            const message = JSON.parse(event.data);
            // Log apenas para mensagens que não sejam game_state
            if (message.type !== 'game_state') {
                addLog(`Recebido: ${event.data}`, 'received');
            }
            console.log('Mensagem processada:', message);
            handleServerMessage(message);
        } catch (e) {
            addLog(`Erro ao processar mensagem: ${e}`, 'error');
            console.error('Erro JSON:', e, 'Data:', event.data);
        }
    };
    
    ws.onclose = function() {
        addLog('Conexão fechada', 'error');
        setConnectedState(false);
    };
    
    ws.onerror = function(error) {
        addLog(`Erro de conexão: ${error}`, 'error');
        console.error(error);
        console.error(error.message)
        setConnectedState(false);
    };
}

function disconnect() {
    if (ws) {
        ws.close();
    }
}

function setConnectedState(connected) {
    isConnected = connected;
    
    if (connected) {
        statusDiv.textContent = 'Conectado';
        statusDiv.className = 'status connected';
        connectBtn.textContent = 'Desconectar';
    } else {
        statusDiv.textContent = 'Desconectado';
        statusDiv.className = 'status disconnected';
        connectBtn.textContent = 'Conectar';
        gameState = null;
        clearCanvas();
        
        // Esconde modal e sidebar quando desconecta
        hideGameOverModal();
        highscoresSidebar.style.display = 'none';
    }
    
    // Atualiza estado dos botões
    resetBtn.disabled = !connected;
    upBtn.disabled = !connected;
    downBtn.disabled = !connected;
    leftBtn.disabled = !connected;
    rightBtn.disabled = !connected;
    
    // Desabilita campos de configuração quando conectado
    gameIdInput.disabled = connected;
    gameWidthInput.disabled = connected;
    gameHeightInput.disabled = connected;
    
    // O slider de velocidade fica habilitado mesmo quando conectado
    // para permitir mudanças em tempo real
}

function sendMessage(message) {
    if (ws && isConnected) {
        const jsonMessage = JSON.stringify(message);
        ws.send(jsonMessage);
        addLog(`Enviado: ${jsonMessage}`, 'sent');
    }
}

// ============================================================================
// FUNÇÕES DE CONTROLE DO JOGO
// ============================================================================

function sendDirection(direction) {
    if (!isConnected || !gameState || gameState.game_over) {
        return;
    }
    
    sendMessage({
        type: 'input',
        direction: direction
    });
}

function resetGame() {
    if (!isConnected) return;
    
    addLog('Resetando jogo...', 'info');
    
    // Esconde o modal se estiver aberto
    hideGameOverModal();
    
    // Limpa o estado local
    gameState = null;
    scoreSpan.textContent = '0';
    clearCanvas();
    
    // Atualiza o status
    if (isConnected) {
        statusDiv.textContent = 'Conectado';
        statusDiv.className = 'status connected';
    }
    
    // Reinicia o jogo com as configurações especificadas
    sendMessage(getJoinGameMessage());
}

// ============================================================================
// TRATAMENTO DE MENSAGENS DO SERVIDOR
// ============================================================================

function handleServerMessage(message) {
    switch (message.type) {
        case 'game_state':
            // O backend envia a estrutura completa do jogo
            gameState = {
                snake: message.snake,
                food: message.food,
                score: message.score,
                game_over: message.game_over,
                width: message.width,
                height: message.height
            };
            
            // Atualiza velocidade se presente no game_state
            if (message.interval !== undefined) {
                updateSpeedFromServer(message.interval);
            }
            
            updateUI();
            drawGame();
            break;
            
        case 'error':
            addLog(`Erro do servidor: ${message.message}`, 'error');
            break;
            
        case 'pong':
            addLog('Pong recebido', 'received');
            break;
            
        case 'connected':
            addLog(`Conectado com ID: ${message.client_id}`, 'received');
            break;
            
        case 'game_reset':
            addLog('Jogo resetado pelo servidor', 'received');
            gameState = null;
            scoreSpan.textContent = '0';
            clearCanvas();
            break;
            
        case 'highscores':
            addLog('Highscores recebidos', 'received');
            renderHighscores(message.highscores);
            break;
    }
}

function updateUI() {
    if (!gameState) return;
    
    scoreSpan.textContent = gameState.score;
    
    if (gameState.game_over) {
        statusDiv.textContent = `Game Over! Pontuação: ${gameState.score}`;
        statusDiv.className = 'status game-over';
        
        // Mostra modal para input do nome (apenas uma vez)
        if (gameOverModal.style.display === 'none') {
            showGameOverModal(gameState.score);
        }
    } else if (isConnected) {
        statusDiv.textContent = 'Jogando';
        statusDiv.className = 'status connected';
    }
}

// ============================================================================
// RENDERIZAÇÃO DO JOGO
// ============================================================================

function clearCanvas() {
    ctx.fillStyle = '#2d2d2d';
    ctx.fillRect(0, 0, canvas.width, canvas.height);
}

function drawGame() {
    if (!gameState) {
        clearCanvas();
        return;
    }
    
    // Limpa canvas
    clearCanvas();
    
    // Desenha grade (opcional)
    drawGrid();
    
    // Desenha comida
    if (gameState.food && gameState.food.position) {
        drawFood();
    }
    
    // Desenha cobra
    if (gameState.snake && gameState.snake.body) {
        drawSnake();
    }
    
    // Desenha game over se necessário
    if (gameState.game_over) {
        drawGameOver();
    }
}

function drawGrid() {
    ctx.strokeStyle = '#404040';
    ctx.lineWidth = 1;
    
    for (let x = 0; x <= canvas.width; x += cellSize) {
        ctx.beginPath();
        ctx.moveTo(x, 0);
        ctx.lineTo(x, canvas.height);
        ctx.stroke();
    }
    
    for (let y = 0; y <= canvas.height; y += cellSize) {
        ctx.beginPath();
        ctx.moveTo(0, y);
        ctx.lineTo(canvas.width, y);
        ctx.stroke();
    }
}

function drawSnake() {
    if (!gameState.snake || !gameState.snake.body) {
        return;
    }
    
    gameState.snake.body.forEach((segment, index) => {
        const x = segment.x * cellSize;
        const y = segment.y * cellSize;
        
        if (index === 0) {
            // Cabeça da cobra
            ctx.fillStyle = '#4CAF50';
        } else {
            // Corpo da cobra
            ctx.fillStyle = '#8BC34A';
        }
        
        ctx.fillRect(x, y, cellSize - 1, cellSize - 1);
        
        // Desenha olhos na cabeça
        if (index === 0) {
            ctx.fillStyle = 'white';
            const eyeSize = 3;
            ctx.fillRect(x + 3, y + 3, eyeSize, eyeSize);
            ctx.fillRect(x + cellSize - 6, y + 3, eyeSize, eyeSize);
        }
    });
}

function drawFood() {
    if (!gameState.food || !gameState.food.position) {
        return;
    }
    
    const x = gameState.food.position.x * cellSize;
    const y = gameState.food.position.y * cellSize;
    
    ctx.fillStyle = '#FF5722';
    ctx.fillRect(x, y, cellSize - 1, cellSize - 1);
    
    // Adiciona brilho à comida
    ctx.fillStyle = '#FF8A65';
    ctx.fillRect(x + 2, y + 2, cellSize - 5, cellSize - 5);
}

function drawGameOver() {
    ctx.fillStyle = 'rgba(0, 0, 0, 0.7)';
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    
    ctx.fillStyle = 'white';
    ctx.font = 'bold 24px Arial';
    ctx.textAlign = 'center';
    ctx.fillText('GAME OVER', canvas.width / 2, canvas.height / 2 - 20);
    
    ctx.font = '16px Arial';
    ctx.fillText(`Pontuação Final: ${gameState.score}`, canvas.width / 2, canvas.height / 2 + 10);
    
    ctx.font = '14px Arial';
    ctx.fillText('Pressione "R" ou clique em "Reset" para jogar novamente', canvas.width / 2, canvas.height / 2 + 40);
}

// ============================================================================
// EVENT LISTENERS
// ============================================================================

// Botões
connectBtn.addEventListener('click', function() {
    if (isConnected) {
        disconnect();
    } else {
        connect();
    }
});

resetBtn.addEventListener('click', resetGame);

// Botões direcionais
upBtn.addEventListener('click', () => sendDirection('Up'));
downBtn.addEventListener('click', () => sendDirection('Down'));
leftBtn.addEventListener('click', () => sendDirection('Left'));
rightBtn.addEventListener('click', () => sendDirection('Right'));

// Slider de velocidade
gameSpeedSlider.addEventListener('input', function() {
    updateSpeedDisplay();
});

gameSpeedSlider.addEventListener('change', function() {
    sendSpeedChange();
});

// Modal de Game Over
submitScoreBtn.addEventListener('click', submitUsername);
skipScoreBtn.addEventListener('click', skipScore);

// Enter no input do username
usernameInput.addEventListener('keydown', function(event) {
    if (event.key === 'Enter') {
        event.preventDefault();
        submitUsername();
    }
    if (event.key === 'Escape') {
        event.preventDefault();
        skipScore();
    }
});

// Teclado
document.addEventListener('keydown', function(event) {
    // Permite reset em qualquer situação quando conectado
    if (isConnected && (event.key === 'r' || event.key === 'R')) {
        event.preventDefault();
        resetGame();
        return;
    }
    
    // Controles de velocidade com + e -
    if (isConnected && !isUpdatingSpeedFromServer) {
        if (event.key === '+' || event.key === '=') {
            event.preventDefault();
            const currentSpeed = parseInt(gameSpeedSlider.value);
            const newSpeed = Math.max(50, currentSpeed - 50); // Mais rápido = menor intervalo
            gameSpeedSlider.value = newSpeed;
            updateSpeedDisplay();
            sendSpeedChange();
            return;
        }
        if (event.key === '-' || event.key === '_') {
            event.preventDefault();
            const currentSpeed = parseInt(gameSpeedSlider.value);
            const newSpeed = Math.min(2000, currentSpeed + 50); // Mais lento = maior intervalo
            gameSpeedSlider.value = newSpeed;
            updateSpeedDisplay();
            sendSpeedChange();
            return;
        }
    }
    
    // Só processa direções se o jogo estiver ativo
    if (!isConnected || !gameState || gameState.game_over) {
        return;
    }
    
    switch (event.key) {
        case 'ArrowUp':
        case 'w':
        case 'W':
            event.preventDefault();
            sendDirection('Up');
            break;
        case 'ArrowDown':
        case 's':
        case 'S':
            event.preventDefault();
            sendDirection('Down');
            break;
        case 'ArrowLeft':
        case 'a':
        case 'A':
            event.preventDefault();
            sendDirection('Left');
            break;
        case 'ArrowRight':
        case 'd':
        case 'D':
            event.preventDefault();
            sendDirection('Right');
            break;
    }
});

// ============================================================================
// INICIALIZAÇÃO
// ============================================================================

// Inicializa canvas
clearCanvas();
updateSpeedDisplay(); // Inicializa o display da velocidade
addLog('Cliente carregado. Clique em "Conectar" para começar!', 'info');
addLog('Controles: Setas/WASD para mover, R para reset, +/- para velocidade', 'info');

// Auto-conecta (opcional)
// connect();
